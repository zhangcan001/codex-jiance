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

use super::model::{ProjectUsageAggregate, ProjectUsageReport};

const MAX_PROJECTS: usize = 100;
const DERIVED_TRUST: &str = "derived";

pub(crate) struct ProjectService {
    repository: Arc<DesktopRepository>,
}

impl ProjectService {
    pub(crate) fn new(repository: Arc<DesktopRepository>) -> Self {
        Self { repository }
    }

    pub(crate) async fn get_usage(
        &self,
        start_at: Option<i64>,
        end_at: Option<i64>,
    ) -> Result<ProjectUsageReport, AppError> {
        let rows = sqlx::query(
            "SELECT project_key, project_name, thread_id, model_id, observed_at,
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
        let mut unknown_project_events = 0usize;
        for row in rows {
            let project_key: Option<String> = row.try_get("project_key")?;
            let project_name: Option<String> = row.try_get("project_name")?;
            let key = project_key.unwrap_or_else(|| "unknown".to_owned());
            let name = project_name.unwrap_or_else(|| "Unknown".to_owned());
            if key == "unknown" {
                unknown_project_events += 1;
            }
            let entry = aggregates
                .entry(key.clone())
                .or_insert_with(|| AggregateBuilder::new(key, name));
            entry.add(&row)?;
        }

        let observed_delta_events = aggregates
            .values()
            .map(|aggregate| aggregate.event_count)
            .sum();
        let priced_event_count = aggregates
            .values()
            .map(|aggregate| aggregate.priced_event_count)
            .sum();
        let mut projects = aggregates
            .into_values()
            .map(AggregateBuilder::finish)
            .collect::<Result<Vec<_>, _>>()?;
        projects.sort_by(|left, right| {
            right
                .total_tokens
                .cmp(&left.total_tokens)
                .then_with(|| left.project_name.cmp(&right.project_name))
        });
        projects.truncate(MAX_PROJECTS);
        let pricing_coverage_percent = coverage(priced_event_count, observed_delta_events);
        Ok(ProjectUsageReport {
            projects,
            observed_delta_events,
            unknown_project_events,
            pricing_coverage_percent,
            start_at,
            end_at,
        })
    }
}

struct AggregateBuilder {
    project_key: String,
    project_name: String,
    threads: HashSet<String>,
    event_count: usize,
    total_tokens: u64,
    input_tokens: u64,
    cached_input_tokens: u64,
    cache_write_input_tokens: u64,
    output_tokens: u64,
    reasoning_output_tokens: u64,
    priced_event_count: usize,
    unpriced_event_count: usize,
    cost: f64,
    first_observed_at: Option<i64>,
    last_observed_at: Option<i64>,
}

impl AggregateBuilder {
    fn new(project_key: String, project_name: String) -> Self {
        Self {
            project_key,
            project_name,
            threads: HashSet::new(),
            event_count: 0,
            total_tokens: 0,
            input_tokens: 0,
            cached_input_tokens: 0,
            cache_write_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            priced_event_count: 0,
            unpriced_event_count: 0,
            cost: 0.0,
            first_observed_at: None,
            last_observed_at: None,
        }
    }

    fn add(&mut self, row: &sqlx::sqlite::SqliteRow) -> Result<(), AppError> {
        self.event_count += 1;
        self.threads.insert(row.try_get("thread_id")?);
        let total = positive(row.try_get("delta_total_tokens")?, "delta total tokens")?;
        let input = positive(row.try_get("delta_input_tokens")?, "delta input tokens")?;
        let cached = positive(
            row.try_get("delta_cached_input_tokens")?,
            "delta cached input tokens",
        )?;
        let cache_write = positive(
            row.try_get("delta_cache_write_input_tokens")?,
            "delta cache write input tokens",
        )?;
        let output = positive(row.try_get("delta_output_tokens")?, "delta output tokens")?;
        let reasoning = positive(
            row.try_get("delta_reasoning_output_tokens")?,
            "delta reasoning output tokens",
        )?;
        self.total_tokens = self.total_tokens.saturating_add(total);
        self.input_tokens = self.input_tokens.saturating_add(input);
        self.cached_input_tokens = self.cached_input_tokens.saturating_add(cached);
        self.cache_write_input_tokens = self.cache_write_input_tokens.saturating_add(cache_write);
        self.output_tokens = self.output_tokens.saturating_add(output);
        self.reasoning_output_tokens = self.reasoning_output_tokens.saturating_add(reasoning);
        let model_id: Option<String> = row.try_get("model_id")?;
        let has_cache_write_flag = row
            .columns()
            .iter()
            .any(|column| column.name() == "cache_write_telemetry_present");
        let cache_write_complete = !has_cache_write_flag
            || row
                .try_get::<i64, _>("cache_write_telemetry_present")
                .unwrap_or(1)
                != 0;
        if cache_write_complete {
            if let Some(model) = model_id {
                let uncached = valid_uncached_input(input, cached, cache_write);
                if let Some(uncached) = uncached {
                    if let Ok(cost) = calculate_api_equivalent_cost(TokenCostInput {
                        model,
                        uncached_input_tokens: uncached,
                        cached_input_tokens: cached,
                        cache_write_input_tokens: cache_write,
                        output_tokens: output,
                    }) {
                        self.cost += cost.total_usd;
                        self.priced_event_count += 1;
                    } else {
                        self.unpriced_event_count += 1;
                    }
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

    fn finish(self) -> Result<ProjectUsageAggregate, AppError> {
        let cache_hit_percent = if self.input_tokens > 0 {
            Some(self.cached_input_tokens as f64 / self.input_tokens as f64 * 100.0)
        } else {
            None
        };
        Ok(ProjectUsageAggregate {
            project_key: self.project_key,
            project_name: self.project_name,
            thread_count: self.threads.len(),
            observed_event_count: self.event_count,
            attributed_delta_event_count: self.event_count,
            total_tokens: self.total_tokens,
            input_tokens: self.input_tokens,
            cached_input_tokens: self.cached_input_tokens,
            cache_write_input_tokens: self.cache_write_input_tokens,
            output_tokens: self.output_tokens,
            reasoning_output_tokens: self.reasoning_output_tokens,
            cache_hit_percent,
            api_equivalent_cost_usd: (self.priced_event_count > 0).then_some(self.cost),
            priced_event_count: self.priced_event_count,
            unpriced_event_count: self.unpriced_event_count,
            pricing_coverage_percent: coverage(self.priced_event_count, self.event_count),
            first_observed_at: self.first_observed_at,
            last_observed_at: self.last_observed_at,
            trust_class: DERIVED_TRUST.to_owned(),
        })
    }
}

fn positive(value: i64, name: &str) -> Result<u64, AppError> {
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
    fn pricing_coverage_is_percentage_of_events() {
        assert_eq!(coverage(1, 2), 50.0);
        assert_eq!(coverage(0, 0), 0.0);
    }

    #[test]
    fn invalid_input_breakdown_cannot_create_uncached_tokens() {
        assert_eq!(valid_uncached_input(10, 6, 5), None);
        assert_eq!(valid_uncached_input(u64::MAX, u64::MAX, 1), None);
        assert_eq!(valid_uncached_input(10, 6, 4), Some(0));
    }

    #[tokio::test]
    async fn invalid_breakdown_keeps_totals_but_excludes_pricing() {
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

        let mut aggregate =
            AggregateBuilder::new("C:\\Projects\\Demo".to_owned(), "Demo".to_owned());
        aggregate
            .add(&row)
            .expect("aggregate should accept token totals");
        let aggregate = aggregate.finish().expect("aggregate should finish");

        assert_eq!(aggregate.total_tokens, 100);
        assert_eq!(aggregate.input_tokens, 10);
        assert_eq!(aggregate.priced_event_count, 0);
        assert_eq!(aggregate.unpriced_event_count, 1);
    }
}
