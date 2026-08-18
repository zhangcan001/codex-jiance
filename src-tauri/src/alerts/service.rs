use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
};

use tauri::AppHandle;
use tauri_plugin_notification::{NotificationExt, PermissionState};

use crate::{
    desktop::DesktopRateLimitService,
    prediction::{QuotaPrediction, QuotaPredictionOutcome, QuotaPredictionService},
    rate_limit::{RateLimitInfo, RateLimitWindow, RateLimitWindowKind},
    settings::{AppSettings, SettingsService},
    time::unix_timestamp,
};

use super::model::{AlertServiceStatus, QuotaAlert, QuotaAlertSeverity, QuotaAlertType};

const ALERT_HISTORY_LIMIT: usize = 50;
const EXHAUSTED_THRESHOLD: u8 = 100;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OwnedAlertCycleKey {
    limit_id: String,
    window_kind: RateLimitWindowKind,
    window_duration_mins: Option<i64>,
    resets_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ThresholdTrigger {
    threshold: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotificationPermission {
    Granted,
    Denied,
    Prompt,
}

pub(crate) trait AlertNotifier: Send + Sync {
    fn permission_state(&self) -> Result<NotificationPermission, String>;
    fn request_permission(&self) -> Result<NotificationPermission, String>;
    fn notify(&self, title: &str, body: &str) -> Result<(), String>;
}

struct NativeAlertNotifier {
    app: AppHandle,
}

impl AlertNotifier for NativeAlertNotifier {
    fn permission_state(&self) -> Result<NotificationPermission, String> {
        self.app
            .notification()
            .permission_state()
            .map(map_permission)
            .map_err(|error| error.to_string())
    }

    fn request_permission(&self) -> Result<NotificationPermission, String> {
        self.app
            .notification()
            .request_permission()
            .map(map_permission)
            .map_err(|error| error.to_string())
    }

    fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        self.app
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|error| error.to_string())
    }
}

fn map_permission(permission: PermissionState) -> NotificationPermission {
    match permission {
        PermissionState::Granted => NotificationPermission::Granted,
        PermissionState::Denied => NotificationPermission::Denied,
        PermissionState::Prompt | PermissionState::PromptWithRationale => {
            NotificationPermission::Prompt
        }
    }
}

struct AlertStore {
    history: VecDeque<QuotaAlert>,
    processed_thresholds: HashSet<(OwnedAlertCycleKey, u8)>,
    processed_predictions: HashSet<OwnedAlertCycleKey>,
    deferred_predictions: HashMap<OwnedAlertCycleKey, QuotaAlert>,
    next_id: u64,
}

impl AlertStore {
    fn new() -> Self {
        Self {
            history: VecDeque::new(),
            processed_thresholds: HashSet::new(),
            processed_predictions: HashSet::new(),
            deferred_predictions: HashMap::new(),
            next_id: 1,
        }
    }

    fn add(&mut self, mut alert: QuotaAlert) {
        alert.id = format!("alert-{}", self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.history.push_back(alert);
        while self.history.len() > ALERT_HISTORY_LIMIT {
            self.history.pop_front();
        }
    }
}

pub(crate) struct AlertService {
    rate_limit_service: Arc<DesktopRateLimitService>,
    prediction_service: Arc<QuotaPredictionService>,
    settings_service: Arc<SettingsService>,
    notifier: Arc<dyn AlertNotifier>,
    store: Mutex<AlertStore>,
    worker: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    running: Mutex<bool>,
}

impl AlertService {
    pub(crate) fn new(
        app: AppHandle,
        rate_limit_service: Arc<DesktopRateLimitService>,
        prediction_service: Arc<QuotaPredictionService>,
        settings_service: Arc<SettingsService>,
    ) -> Arc<Self> {
        Arc::new(Self::with_notifier(
            rate_limit_service,
            prediction_service,
            Arc::new(NativeAlertNotifier { app }),
            settings_service,
        ))
    }

    fn with_notifier(
        rate_limit_service: Arc<DesktopRateLimitService>,
        prediction_service: Arc<QuotaPredictionService>,
        notifier: Arc<dyn AlertNotifier>,
        settings_service: Arc<SettingsService>,
    ) -> Self {
        Self {
            rate_limit_service,
            prediction_service,
            settings_service,
            notifier,
            store: Mutex::new(AlertStore::new()),
            worker: Mutex::new(None),
            running: Mutex::new(false),
        }
    }

    pub(crate) fn start(self: &Arc<Self>) {
        let Ok(mut worker) = self.worker.lock() else {
            log::error!("Alert worker state is poisoned; alert worker was not started");
            return;
        };
        if worker
            .as_ref()
            .is_some_and(|task| !task.inner().is_finished())
        {
            return;
        }
        let receiver = self.rate_limit_service.subscribe_updates();
        let service = Arc::clone(self);
        let Ok(mut running) = self.running.lock() else {
            log::error!("Alert running state is poisoned; alert worker was not started");
            return;
        };
        *running = true;
        *worker = Some(tauri::async_runtime::spawn(async move {
            service.run(receiver).await;
        }));
    }

    pub(crate) async fn shutdown(&self) {
        let task = match self.worker.lock() {
            Ok(mut worker) => worker.take(),
            Err(_) => {
                log::error!("Alert worker state is poisoned during shutdown");
                None
            }
        };
        if let Some(task) = task {
            task.abort();
            let _ = task.await;
        }
        if let Ok(mut running) = self.running.lock() {
            *running = false;
        } else {
            log::error!("Alert running state is poisoned during shutdown");
        }
    }

    async fn run(self: Arc<Self>, mut receiver: tokio::sync::broadcast::Receiver<RateLimitInfo>) {
        loop {
            match receiver.recv().await {
                Ok(info) => self.process_update(info).await,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    log::warn!(
                        "Alert worker lagged by {skipped} rate-limit updates; refreshing once"
                    );
                    let refreshed = self.rate_limit_service.get_rate_limits(true).await;
                    if refreshed.status != crate::rate_limit::RateLimitStatus::Available {
                        log::warn!("Alert worker lagged refresh failed; waiting for a future official update");
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
        if let Ok(mut running) = self.running.lock() {
            *running = false;
        } else {
            log::error!("Alert running state is poisoned after worker exit");
        }
    }

    async fn process_update(&self, info: RateLimitInfo) {
        let predictions = self.prediction_service.get_predictions(false).await;
        let settings = self.settings_service.snapshot().settings;
        let now = unix_timestamp();
        let mut pending_notifications = Vec::new();

        {
            let Ok(mut store) = self.store.lock() else {
                log::error!("Alert store is poisoned; update was ignored");
                return;
            };
            for window in &info.windows {
                let triggers = if settings.usage_threshold_alerts {
                    threshold_triggers(window, &mut store.processed_thresholds, &settings)
                } else {
                    Vec::new()
                };
                let threshold_was_created = !triggers.is_empty();
                for trigger in triggers {
                    let alert = threshold_alert(window, trigger.threshold, &settings, now);
                    store.add(alert.clone());
                    pending_notifications.push((alert, true, None));
                }

                let prediction = predictions.iter().find(|prediction| {
                    prediction.limit_id == window.limit_id
                        && prediction.window_kind == Some(window.window_kind)
                        && prediction.window_duration_mins == window.window_duration_mins
                        && prediction.resets_at == window.resets_at
                });
                if settings.prediction_alerts {
                    if let Some(prediction) =
                        prediction_alert_candidate(prediction, settings.prediction_alert_minutes)
                    {
                        let Some(limit_id) = window.limit_id.as_deref() else {
                            continue;
                        };
                        let cycle = OwnedAlertCycleKey {
                            limit_id: limit_id.to_owned(),
                            window_kind: window.window_kind,
                            window_duration_mins: window.window_duration_mins,
                            resets_at: window.resets_at,
                        };
                        if store.processed_predictions.insert(cycle.clone()) {
                            let alert = prediction_alert(window, prediction, now);
                            store.add(alert.clone());
                            if threshold_was_created {
                                store
                                    .deferred_predictions
                                    .insert(cycle.clone(), alert.clone());
                            }
                            pending_notifications.push((alert, !threshold_was_created, None));
                        } else if !threshold_was_created {
                            if let Some(alert) = store.deferred_predictions.get(&cycle).cloned() {
                                pending_notifications.push((alert, true, Some(cycle)));
                            }
                        }
                    }
                }
            }
        }

        let notifications_enabled = settings.system_notifications
            && self
                .notifier
                .permission_state()
                .is_ok_and(|permission| permission == NotificationPermission::Granted);
        for (alert, should_notify, deferred_cycle) in pending_notifications {
            if should_notify && notifications_enabled {
                match self.notifier.notify("Codex 用量监控器", &alert.message) {
                    Ok(()) => {
                        if let Some(cycle) = deferred_cycle {
                            if let Ok(mut store) = self.store.lock() {
                                store.deferred_predictions.remove(&cycle);
                            } else {
                                log::error!(
                                    "Alert store is poisoned while clearing deferred alert"
                                );
                            }
                        }
                    }
                    Err(error) => {
                        log::warn!("Codex alert notification failed: {error}");
                    }
                }
            }
        }
    }

    pub(crate) fn status(&self) -> AlertServiceStatus {
        let permission = self
            .notifier
            .permission_state()
            .unwrap_or(NotificationPermission::Prompt);
        let (alert_count, latest_alerts) = self
            .store
            .lock()
            .map(|store| {
                (
                    store.history.len(),
                    store.history.iter().rev().take(10).cloned().collect(),
                )
            })
            .unwrap_or_else(|_| (0, Vec::new()));
        let active_worker = self
            .worker
            .lock()
            .ok()
            .and_then(|worker| worker.as_ref().map(|task| !task.inner().is_finished()))
            .unwrap_or(false);
        AlertServiceStatus {
            running: self.running.lock().map(|running| *running).unwrap_or(false),
            notification_permission: permission_name(permission).to_owned(),
            notification_available: permission == NotificationPermission::Granted,
            active_worker,
            alert_count,
            latest_alerts,
        }
    }

    pub(crate) async fn request_notification_permission(&self) -> AlertServiceStatus {
        if let Err(error) = self.notifier.request_permission() {
            log::warn!("Notification permission request failed: {error}");
        }
        self.status()
    }
}

fn permission_name(permission: NotificationPermission) -> &'static str {
    match permission {
        NotificationPermission::Granted => "granted",
        NotificationPermission::Denied => "denied",
        NotificationPermission::Prompt => "prompt",
    }
}

fn cycle_key(window: &RateLimitWindow) -> Option<OwnedAlertCycleKey> {
    Some(OwnedAlertCycleKey {
        limit_id: window.limit_id.clone()?,
        window_kind: window.window_kind,
        window_duration_mins: window.window_duration_mins,
        resets_at: window.resets_at,
    })
}

fn threshold_triggers(
    window: &RateLimitWindow,
    processed: &mut HashSet<(OwnedAlertCycleKey, u8)>,
    settings: &AppSettings,
) -> Vec<ThresholdTrigger> {
    let Some(cycle) = cycle_key(window) else {
        return Vec::new();
    };
    let thresholds = [
        settings.warning_threshold,
        settings.high_threshold,
        settings.critical_threshold,
        EXHAUSTED_THRESHOLD,
    ];
    let crossed = thresholds
        .into_iter()
        .filter(|threshold| window.used_percent >= f64::from(*threshold))
        .collect::<Vec<_>>();
    let new_thresholds = crossed
        .iter()
        .copied()
        .filter(|threshold| !processed.contains(&(cycle.clone(), *threshold)))
        .collect::<Vec<_>>();
    let Some(highest_new) = new_thresholds.last().copied() else {
        return Vec::new();
    };
    for threshold in crossed {
        if threshold <= highest_new {
            processed.insert((cycle.clone(), threshold));
        }
    }
    vec![ThresholdTrigger {
        threshold: highest_new,
    }]
}

fn prediction_alert_candidate(
    prediction: Option<&QuotaPrediction>,
    alert_minutes: u16,
) -> Option<&QuotaPrediction> {
    let prediction = prediction?;
    if prediction.outcome != QuotaPredictionOutcome::DepletionBeforeReset {
        return None;
    }
    let seconds = prediction.seconds_to_depletion?;
    let max_seconds = f64::from(alert_minutes) * 60.0;
    (seconds.is_finite() && (0.0..=max_seconds).contains(&seconds)).then_some(prediction)
}

fn threshold_alert(
    window: &RateLimitWindow,
    threshold: u8,
    settings: &AppSettings,
    created_at: i64,
) -> QuotaAlert {
    QuotaAlert {
        id: String::new(),
        alert_type: QuotaAlertType::UsageThreshold,
        severity: match threshold {
            value if value == settings.warning_threshold => QuotaAlertSeverity::Warning,
            value if value == settings.high_threshold => QuotaAlertSeverity::High,
            value if value == settings.critical_threshold => QuotaAlertSeverity::Critical,
            _ => QuotaAlertSeverity::Exhausted,
        },
        limit_id: window.limit_id.clone(),
        limit_name: window.limit_name.clone(),
        window_kind: Some(window.window_kind),
        window_duration_mins: window.window_duration_mins,
        used: Some(window.used_percent),
        threshold: Some(f64::from(threshold)),
        prediction_outcome: None,
        seconds_to_depletion: None,
        resets_at: window.resets_at,
        trust_class: "official".to_owned(),
        created_at,
        message: format!("Codex 额度使用已达到 {threshold}%"),
    }
}

fn prediction_alert(
    window: &RateLimitWindow,
    prediction: &QuotaPrediction,
    created_at: i64,
) -> QuotaAlert {
    QuotaAlert {
        id: String::new(),
        alert_type: QuotaAlertType::PredictedDepletion,
        severity: QuotaAlertSeverity::Critical,
        limit_id: window.limit_id.clone(),
        limit_name: window.limit_name.clone(),
        window_kind: Some(window.window_kind),
        window_duration_mins: window.window_duration_mins,
        used: Some(window.used_percent),
        threshold: None,
        prediction_outcome: Some(prediction.outcome),
        seconds_to_depletion: prediction.seconds_to_depletion,
        resets_at: window.resets_at,
        trust_class: "estimated".to_owned(),
        created_at,
        message: "按当前消耗速度，额度可能在重置前耗尽".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::rate_limit::RateLimitWindowKind;

    fn window(used_percent: f64, resets_at: Option<i64>) -> RateLimitWindow {
        RateLimitWindow {
            limit_id: Some("chatgpt".to_owned()),
            limit_name: Some("ChatGPT".to_owned()),
            window_kind: RateLimitWindowKind::Primary,
            used_percent,
            remaining_percent: 100.0 - used_percent,
            window_duration_mins: Some(300),
            resets_at,
            plan_type: None,
            rate_limit_reached_type: None,
        }
    }

    #[test]
    fn threshold_rules_progress_and_skip_lower_first_alerts() {
        let mut processed = HashSet::new();
        let settings = AppSettings::default();
        assert_eq!(
            threshold_triggers(&window(79.0, Some(1)), &mut processed, &settings).len(),
            0
        );
        assert_eq!(
            threshold_triggers(&window(82.0, Some(1)), &mut processed, &settings)[0].threshold,
            80
        );
        assert_eq!(
            threshold_triggers(&window(91.0, Some(1)), &mut processed, &settings)[0].threshold,
            90
        );
        assert_eq!(
            threshold_triggers(&window(96.0, Some(1)), &mut processed, &settings)[0].threshold,
            95
        );
        assert_eq!(
            threshold_triggers(&window(100.0, Some(1)), &mut processed, &settings)[0].threshold,
            100
        );
        assert!(threshold_triggers(&window(100.0, Some(1)), &mut processed, &settings).is_empty());
    }

    #[test]
    fn first_high_usage_emits_only_highest_crossed_threshold_and_reset_restarts_cycle() {
        let mut processed = HashSet::new();
        let settings = AppSettings::default();
        assert_eq!(
            threshold_triggers(&window(96.0, Some(1)), &mut processed, &settings)[0].threshold,
            95
        );
        assert_eq!(
            threshold_triggers(&window(96.0, Some(2)), &mut processed, &settings)[0].threshold,
            95
        );
    }

    #[test]
    fn custom_thresholds_keep_exhausted_fixed_at_100() {
        let mut settings = AppSettings::default();
        settings.warning_threshold = 70;
        settings.high_threshold = 85;
        settings.critical_threshold = 93;
        let mut processed = HashSet::new();
        assert_eq!(
            threshold_triggers(&window(90.0, Some(1)), &mut processed, &settings)[0].threshold,
            85
        );
        assert_eq!(
            threshold_triggers(&window(100.0, Some(1)), &mut processed, &settings)[0].threshold,
            100
        );
        assert!(threshold_triggers(&window(100.0, Some(1)), &mut processed, &settings).is_empty());
        assert_eq!(
            threshold_alert(&window(70.0, Some(1)), 70, &settings, 1).severity,
            QuotaAlertSeverity::Warning
        );
    }

    #[test]
    fn prediction_alert_requires_one_hour_and_is_deduplicated_by_cycle() {
        let prediction = QuotaPrediction {
            outcome: QuotaPredictionOutcome::DepletionBeforeReset,
            limit_id: Some("chatgpt".to_owned()),
            limit_name: Some("ChatGPT".to_owned()),
            window_kind: Some(RateLimitWindowKind::Primary),
            window_duration_mins: Some(300),
            used_percent: Some(90.0),
            burn_rate_percent_points_per_hour: Some(20.0),
            estimated_depletion_at: Some(1_000),
            seconds_to_depletion: Some(3_600.0),
            resets_at: Some(2_000),
            confidence: crate::prediction::PredictionConfidence::Low,
            trust_class: "estimated".to_owned(),
            calculated_at: 0,
            message: Some("Estimated depletion before reset".to_owned()),
        };
        assert!(prediction_alert_candidate(Some(&prediction), 60).is_some());
        let mut too_late = prediction.clone();
        too_late.seconds_to_depletion = Some(3_601.0);
        assert!(prediction_alert_candidate(Some(&too_late), 60).is_none());
        assert!(prediction_alert_candidate(Some(&prediction), 5).is_none());
    }

    struct FakeNotifier {
        fail: bool,
        calls: Mutex<usize>,
    }

    impl AlertNotifier for FakeNotifier {
        fn permission_state(&self) -> Result<NotificationPermission, String> {
            Ok(NotificationPermission::Granted)
        }

        fn request_permission(&self) -> Result<NotificationPermission, String> {
            Ok(NotificationPermission::Granted)
        }

        fn notify(&self, _title: &str, _body: &str) -> Result<(), String> {
            *self
                .calls
                .lock()
                .expect("fake notifier mutex should not be poisoned") += 1;
            if self.fail {
                Err("fake notification failure".to_owned())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn alert_history_is_capped_and_survives_notifier_failure() {
        let notifier = FakeNotifier {
            fail: true,
            calls: Mutex::new(0),
        };
        let mut store = AlertStore::new();
        let settings = AppSettings::default();
        for index in 0..51 {
            store.add(threshold_alert(
                &window(80.0, Some(1)),
                80,
                &settings,
                index,
            ));
        }
        assert_eq!(store.history.len(), ALERT_HISTORY_LIMIT);
        assert!(notifier.notify("title", "body").is_err());
        assert_eq!(
            *notifier
                .calls
                .lock()
                .expect("fake notifier mutex should not be poisoned"),
            1
        );
        assert_eq!(store.history.len(), ALERT_HISTORY_LIMIT);
    }

    #[test]
    fn exhausted_usage_does_not_create_a_prediction_candidate() {
        let prediction = QuotaPrediction {
            outcome: QuotaPredictionOutcome::AlreadyDepleted,
            limit_id: Some("chatgpt".to_owned()),
            limit_name: Some("ChatGPT".to_owned()),
            window_kind: Some(RateLimitWindowKind::Primary),
            window_duration_mins: Some(300),
            used_percent: Some(100.0),
            burn_rate_percent_points_per_hour: None,
            estimated_depletion_at: None,
            seconds_to_depletion: None,
            resets_at: Some(2_000),
            confidence: crate::prediction::PredictionConfidence::Low,
            trust_class: "estimated".to_owned(),
            calculated_at: 0,
            message: Some("Quota currently exhausted".to_owned()),
        };
        assert!(prediction_alert_candidate(Some(&prediction), 60).is_none());
    }
}
