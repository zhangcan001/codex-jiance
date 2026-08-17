mod model;
mod service;

pub(crate) use model::{QuotaPrediction, QuotaPredictionOutcome};

#[cfg(test)]
pub(crate) use model::PredictionConfidence;
pub(crate) use service::QuotaPredictionService;
