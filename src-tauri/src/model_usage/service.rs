use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use sqlx::{Column, Row};

use crate::{
    desktop::DesktopRepository,
    error::AppError,
    pricing::{calculate_api_equivalent_cost, TokenCostInput},
};

use super::model::{ModelUsageAggregate, ModelUsageReport};

const MAX_MODELS: usize = 100;
const DERIVED_TRUST: &str = "derived";

pub(crate) struct ModelUsageService {
    repository: Arc<DesktopRepository>,
}

impl ModelUsageService {
    pub(crate) fn new(repository: Arc<DesktopRepository>) -> Self {
        Self { repository }
    }

    pub(crate) async fn get_usage(
        &self,
        start_at: Option<i64>,
        end_at: Option<i64>,
    ) -> Result<ModelUsageReport, AppError> {
        let rows = sqlx::query(
            "SELECT model_id, model_source, thread_id, observed_at,
                    delta_total_tokens, delta_input_tokens, delta_cached_input_tokens,
                    delta_cache_write_input_tokens, delta_output_tokens,
                    delta_reasoning_output_tokens, cache_write_telemetry_present
             FROM thread_token_snapshots
             WHERE source='desktop_rollout' AND delta_total_tokens IS NOT NULL
               AND (? IS NULL OR observed_at >= ?)
               AND (? IS NULL OR observed_at < ?)
             ORDER BY observed_at ASC, id ASC",
        )
        .bind(start_at)
        .bind(start_at)
        .bind(end_at)
        .bind(end_at)
        .fetch_all(&self.repository.pool)
        .await?;

        let mut aggregates = HashMap::<String, AggregateBuilder>::new();
        for row in rows {
            let model_id: Option<String> = row.try_get("model_id")?;
            let key = model_id.clone().unwrap_or_else(|| "Unknown".to_owned());
            let source: Option<String> = row.try_get("model_source")?;
            let entry = aggregates.entry(key.clone()).or_insert_with(|| {
                AggregateBuilder::new(key, source.unwrap_or_else(|| "unknown".to_owned()))
            });
            entry.add(&row, model_id.as_deref())?;
        }
        let observed_delta_events = aggregates
            .values()
            .map(|aggregate| aggregate.event_count)
            .sum();
        let priced_delta_events = aggregates
            .values()
            .map(|aggregate| aggregate.priced_event_count)
            .sum();
        let total_cost = aggregates
            .values()
            .filter_map(|aggregate| (aggregate.priced_event_count > 0).then_some(aggregate.cost))
            .sum::<f64>();
        let mut models = aggregates
            .into_values()
            .map(AggregateBuilder::finish)
            .collect::<Result<Vec<_>, _>>()?;
        models.sort_by(|left, right| {
            right
                .total_tokens
                .cmp(&left.total_tokens)
                .then_with(|| left.model_id.cmp(&right.model_id))
        });
        models.truncate(MAX_MODELS);
        Ok(ModelUsageReport {
            models,
            observed_delta_events,
            priced_delta_events,
            pricing_coverage_percent: coverage(priced_delta_events, observed_delta_events),
            total_api_equivalent_cost_usd: (priced_delta_events > 0).then_some(total_cost),
            start_at,
            end_at,
        })
    }
}

struct AggregateBuilder {
    model_id: String,
    model_source: String,
    threads: HashSet<String>,
    event_count: usize,
    total_tokens: u64,
    input_tokens: u64,
    uncached_input_tokens: u64,
    cached_input_tokens: u64,
    cache_write_input_tokens: u64,
    output_tokens: u64,
    reasoning_output_tokens: u64,
    priced_event_count: usize,
    unpriced_event_count: usize,
    cost: f64,
    pricing_effective_date: Option<String>,
    first_observed_at: Option<i64>,
    last_observed_at: Option<i64>,
}

impl AggregateBuilder {
    fn new(model_id: String, model_source: String) -> Self {
        Self {
            model_id,
            model_source,
            threads: HashSet::new(),
            event_count: 0,
            total_tokens: 0,
            input_tokens: 0,
            uncached_input_tokens: 0,
            cached_input_tokens: 0,
            cache_write_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            priced_event_count: 0,
            unpriced_event_count: 0,
            cost: 0.0,
            pricing_effective_date: None,
            first_observed_at: None,
            last_observed_at: None,
        }
    }

    fn add(
        &mut self,
        row: &sqlx::sqlite::SqliteRow,
        model_id: Option<&str>,
    ) -> Result<(), AppError> {
        self.event_count += 1;
        self.threads.insert(row.try_get("thread_id")?);
        let total = non_negative(row.try_get("delta_total_tokens")?, "delta total tokens")?;
        let input = non_negative(row.try_get("delta_input_tokens")?, "delta input tokens")?;
        let cached = non_negative(
            row.try_get("delta_cached_input_tokens")?,
            "delta cached input tokens",
        )?;
        let cache_write = non_negative(
            row.try_get("delta_cache_write_input_tokens")?,
            "delta cache write input tokens",
        )?;
        let output = non_negative(row.try_get("delta_output_tokens")?, "delta output tokens")?;
        let reasoning = non_negative(
            row.try_get("delta_reasoning_output_tokens")?,
            "delta reasoning output tokens",
        )?;
        let has_cache_write_flag = row
            .columns()
            .iter()
            .any(|column| column.name() == "cache_write_telemetry_present");
        let cache_write_complete = !has_cache_write_flag
            || row
                .try_get::<i64, _>("cache_write_telemetry_present")
                .unwrap_or(1)
                != 0;
        let uncached = cache_write_complete
            .then(|| valid_uncached_input(input, cached, cache_write))
            .flatten();
        self.total_tokens = self.total_tokens.saturating_add(total);
        self.input_tokens = self.input_tokens.saturating_add(input);
        self.cached_input_tokens = self.cached_input_tokens.saturating_add(cached);
        self.cache_write_input_tokens = self.cache_write_input_tokens.saturating_add(cache_write);
        self.output_tokens = self.output_tokens.saturating_add(output);
        self.reasoning_output_tokens = self.reasoning_output_tokens.saturating_add(reasoning);
        if let Some(uncached) = uncached {
            self.uncached_input_tokens = self.uncached_input_tokens.saturating_add(uncached);
            if let Some(model) = model_id {
                if let Ok(cost) = calculate_api_equivalent_cost(TokenCostInput {
                    model: model.to_owned(),
                    uncached_input_tokens: uncached,
                    cached_input_tokens: cached,
                    cache_write_input_tokens: cache_write,
                    output_tokens: output,
                }) {
                    self.cost += cost.total_usd;
                    self.priced_event_count += 1;
                    self.pricing_effective_date = Some(cost.pricing_effective_date.to_owned());
                } else {
                    self.unpriced_event_count += 1;
                }
            } else {
                self.unpriced_event_count += 1;
            }
        } else {
            self.unpriced_event_count += 1;
        }
        let observed_at: i64 = row.try_get("observed_at")?;
        self.first_observed_at = Some(
            self.first_observed_at
                .map_or(observed_at, |first| first.min(observed_at)),
        );
        self.last_observed_at = Some(
            self.last_observed_at
                .map_or(observed_at, |last| last.max(observed_at)),
        );
        Ok(())
    }

    fn finish(self) -> Result<ModelUsageAggregate, AppError> {
        let cache_hit_percent = if self.input_tokens > 0 {
            Some(self.cached_input_tokens as f64 / self.input_tokens as f64 * 100.0)
        } else {
            None
        };
        Ok(ModelUsageAggregate {
            model_id: self.model_id,
            model_source: self.model_source,
            event_count: self.event_count,
            thread_count: self.threads.len(),
            total_tokens: self.total_tokens,
            input_tokens: self.input_tokens,
            uncached_input_tokens: self.uncached_input_tokens,
            cached_input_tokens: self.cached_input_tokens,
            cache_write_input_tokens: self.cache_write_input_tokens,
            output_tokens: self.output_tokens,
            reasoning_output_tokens: self.reasoning_output_tokens,
            cache_hit_percent,
            api_equivalent_cost_usd: (self.priced_event_count > 0).then_some(self.cost),
            pricing_available: self.priced_event_count > 0,
            pricing_effective_date: self.pricing_effective_date,
            priced_event_count: self.priced_event_count,
            unpriced_event_count: self.unpriced_event_count,
            pricing_coverage_percent: coverage(self.priced_event_count, self.event_count),
            first_observed_at: self.first_observed_at,
            last_observed_at: self.last_observed_at,
            trust_class: DERIVED_TRUST.to_owned(),
        })
    }
}

fn non_negative(value: i64, name: &str) -> Result<u64, AppError> {
    u64::try_from(value)
        .map_err(|_| AppError::InvalidState(format!("{name} must be non-negative.")))
}
fn coverage(priced: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        priced as f64 / total as f64 * 100.0
    }
}

fn valid_uncached_input(input: u64, cached: u64, cache_write: u64) -> Option<u64> {
    cached
        .checked_add(cache_write)
        .and_then(|accounted_input| input.checked_sub(accounted_input))
}

#[cfg(test)]
mod tests {
    use super::{coverage, valid_uncached_input, AggregateBuilder};
    use crate::database::connection::create_pool;

    #[test]
    fn zero_priced_events_have_zero_coverage() {
        assert_eq!(coverage(0, 3), 0.0);
    }

    #[test]
    fn invalid_input_breakdown_cannot_create_uncached_tokens() {
        assert_eq!(valid_uncached_input(10, 6, 5), None);
        assert_eq!(valid_uncached_input(u64::MAX, u64::MAX, 1), None);
        assert_eq!(valid_uncached_input(10, 6, 4), Some(0));
    }

    #[tokio::test]
    async fn invalid_breakdown_keeps_totals_but_excludes_uncached_and_pricing() {
        let pool = create_pool("sqlite::memory:")
            .await
            .expect("memory database should connect");
        let row = sqlx::query(
            "SELECT 100 AS delta_total_tokens, 10 AS delta_input_tokens,
                    6 AS delta_cached_input_tokens, 5 AS delta_cache_write_input_tokens,
                    2 AS delta_output_tokens, 1 AS delta_reasoning_output_tokens,
                    'gpt-5' AS model_id, 'thread-1' AS thread_id, 1 AS observed_at",
        )
        .fetch_one(&pool)
        .await
        .expect("token row should be available");

        let mut aggregate = AggregateBuilder::new("gpt-5".to_owned(), "derived".to_owned());
        aggregate
            .add(&row, Some("gpt-5"))
            .expect("aggregate should accept token totals");
        let aggregate = aggregate.finish().expect("aggregate should finish");

        assert_eq!(aggregate.total_tokens, 100);
        assert_eq!(aggregate.input_tokens, 10);
        assert_eq!(aggregate.uncached_input_tokens, 0);
        assert_eq!(aggregate.priced_event_count, 0);
        assert_eq!(aggregate.unpriced_event_count, 1);
    }
}
