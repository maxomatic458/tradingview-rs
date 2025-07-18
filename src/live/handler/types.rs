use crate::{
    Error,
    chart::{DataPoint, StudyOptions, StudyResponseData, SymbolInfo},
    live::handler::message::{Command, LoadingMsg, TradingViewResponse},
    quote::models::QuoteValue,
    websocket::SeriesInfo,
};
use bon::Builder;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use ustr::Ustr;

pub type DataTx = UnboundedSender<TradingViewResponse>;
pub type DataRx = UnboundedReceiver<TradingViewResponse>;

pub type CommandTx = UnboundedSender<Command>;
pub type CommandRx = UnboundedReceiver<Command>;

pub type CallbackFn<T> = Box<dyn Fn(T) + Send + Sync + 'static>;

fn default_callback<T: std::fmt::Debug>(name: &'static str) -> Arc<CallbackFn<T>> {
    Arc::new(Box::new(move |data| {
        tracing::trace!("Callback trigger on {}: {:?}", name, data);
    }))
}

// Macro to generate setter methods assuming method name and field name are the same
macro_rules! event_setter {
    ($name:ident, $param_type:ty) => {
        pub fn $name(mut self, f: impl Fn($param_type) + Send + Sync + 'static) -> Self {
            self.$name = Arc::new(Box::new(f));
            self
        }
    };
    // Variant for tupled parameters
    ($name:ident, ($($param_type_tuple:ty),+)) => {
        pub fn $name(mut self, f: impl Fn(($($param_type_tuple),+)) + Send + Sync + 'static) -> Self {
            self.$name = Arc::new(Box::new(f));
            self
        }
    };
}

#[derive(Clone, Builder)]
pub struct TradingViewHandler {
    #[builder(default= default_callback::<SymbolInfo>("ON_SYMBOL_INFO"))]
    pub on_symbol_info: Arc<CallbackFn<SymbolInfo>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_SERIES_LOADING"))]
    pub on_series_loading: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<(SeriesInfo, Vec<DataPoint>)>("ON_CHART_DATA"))]
    pub on_chart_data: Arc<CallbackFn<(SeriesInfo, Vec<DataPoint>)>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_SERIES_COMPLETED"))]
    pub on_series_completed: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_STUDY_LOADING"))]
    pub on_study_loading: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<(StudyOptions, StudyResponseData)>("ON_STUDY_DATA"))]
    pub on_study_data: Arc<CallbackFn<(StudyOptions, StudyResponseData)>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_STUDY_COMPLETED"))]
    pub on_study_completed: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<QuoteValue>("ON_QUOTE_DATA"))]
    pub on_quote_data: Arc<CallbackFn<QuoteValue>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_QUOTE_COMPLETED"))]
    pub on_quote_completed: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_OK"))]
    pub on_replay_ok: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_POINT"))]
    pub on_replay_point: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_INSTANCE_ID"))]
    pub on_replay_instance_id: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_RESOLUTIONS"))]
    pub on_replay_resolutions: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_DATA_END"))]
    pub on_replay_data_end: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<(Error, Vec<Value>)>("ON_ERROR"))]
    pub on_error: Arc<CallbackFn<(Error, Vec<Value>)>>,

    #[builder(default= default_callback::<(Ustr, Vec<Value>)>("ON_UNKNOWN_EVENT"))]
    pub on_unknown_event: Arc<CallbackFn<(Ustr, Vec<Value>)>>,
}

impl Default for TradingViewHandler {
    fn default() -> Self {
        TradingViewHandler::builder().build()
    }
}

impl TradingViewHandler {
    event_setter!(on_chart_data, (SeriesInfo, Vec<DataPoint>));
    event_setter!(on_quote_data, QuoteValue);
    event_setter!(on_study_data, (StudyOptions, StudyResponseData));
    event_setter!(on_error, (Error, Vec<Value>));
    event_setter!(on_symbol_info, SymbolInfo);
    event_setter!(on_series_completed, Vec<Value>);
    event_setter!(on_series_loading, Vec<Value>);
    event_setter!(on_quote_completed, Vec<Value>);
    event_setter!(on_replay_ok, Vec<Value>);
    event_setter!(on_replay_point, Vec<Value>);
    event_setter!(on_replay_instance_id, Vec<Value>);
    event_setter!(on_replay_resolutions, Vec<Value>);
    event_setter!(on_replay_data_end, Vec<Value>);
    event_setter!(on_study_loading, Vec<Value>);
    event_setter!(on_study_completed, Vec<Value>);
    event_setter!(on_unknown_event, (Ustr, Vec<Value>));
}

pub fn create_handler(tx: Arc<DataTx>) -> TradingViewHandler {
    TradingViewHandler::builder()
        .on_symbol_info({
            let tx = tx.clone();
            Arc::new(Box::new(move |data| {
                let _ = tx.send(TradingViewResponse::SymbolInfo(data));
            }))
        })
        .on_series_loading({
            let tx = tx.clone();
            Arc::new(Box::new(move |data: Vec<Value>| {
                let msg = if let Ok(msg) = LoadingMsg::new(&data) {
                    msg
                } else {
                    tracing::error!("Failed to parse LoadingMsg from data: {:?}", data);
                    return;
                };
                if let Err(e) = tx.send(TradingViewResponse::SeriesLoading(msg)) {
                    tracing::error!("Failed to send SeriesLoading response: {}", e);
                }
            }))
        })
        .on_chart_data({
            let tx = tx.clone();
            Arc::new(Box::new(move |(series_info, data_points)| {
                if let Err(e) = tx.send(TradingViewResponse::ChartData(series_info, data_points)) {
                    tracing::error!("Failed to send ChartData response: {}", e);
                }
            }))
        })
        .on_series_completed({
            let tx = tx.clone();
            Arc::new(Box::new(move |data| {
                if let Err(e) = tx.send(TradingViewResponse::SeriesCompleted(data)) {
                    tracing::error!("Failed to send SeriesCompleted response: {}", e);
                }
            }))
        })
        .on_study_loading({
            let tx = tx.clone();
            Arc::new(Box::new(move |data| {
                let msg = if let Ok(msg) = LoadingMsg::new(&data) {
                    msg
                } else {
                    tracing::error!("Failed to parse LoadingMsg from data: {:?}", data);
                    return;
                };
                if let Err(e) = tx.send(TradingViewResponse::StudyLoading(msg)) {
                    tracing::error!("Failed to send StudyLoading response: {}", e);
                }
            }))
        })
        .on_study_data({
            let tx = tx.clone();
            Arc::new(Box::new(move |(study_options, study_data)| {
                if let Err(e) = tx.send(TradingViewResponse::StudyData(study_options, study_data)) {
                    tracing::error!("Failed to send StudyData response: {}", e);
                }
            }))
        })
        .on_study_completed({
            let tx = tx.clone();
            Arc::new(Box::new(move |data| {
                if let Err(e) = tx.send(TradingViewResponse::StudyCompleted(data)) {
                    tracing::error!("Failed to send StudyCompleted response: {}", e);
                }
            }))
        })
        .on_quote_data({
            let tx = tx.clone();
            Arc::new(Box::new(move |data| {
                if let Err(e) = tx.send(TradingViewResponse::QuoteData(data)) {
                    tracing::error!("Failed to send QuoteData response: {}", e);
                }
            }))
        })
        .on_quote_completed({
            let tx = tx.clone();
            Arc::new(Box::new(move |data| {
                if let Err(e) = tx.send(TradingViewResponse::QuoteCompleted(data)) {
                    tracing::error!("Failed to send QuoteCompleted response: {}", e);
                }
            }))
        })
        .on_replay_ok({
            let tx = tx.clone();
            Arc::new(Box::new(move |data| {
                if let Err(e) = tx.send(TradingViewResponse::ReplayOk(data)) {
                    tracing::error!("Failed to send ReplayOk response: {}", e);
                }
            }))
        })
        .on_replay_point({
            let tx = tx.clone();
            Arc::new(Box::new(move |data| {
                if let Err(e) = tx.send(TradingViewResponse::ReplayPoint(data)) {
                    tracing::error!("Failed to send ReplayPoint response: {}", e);
                }
            }))
        })
        .on_replay_instance_id({
            let tx = tx.clone();
            Arc::new(Box::new(move |data| {
                if let Err(e) = tx.send(TradingViewResponse::ReplayInstanceId(data)) {
                    tracing::error!("Failed to send ReplayInstanceId response: {}", e);
                }
            }))
        })
        .on_replay_resolutions({
            let tx = tx.clone();
            Arc::new(Box::new(move |data| {
                if let Err(e) = tx.send(TradingViewResponse::ReplayResolutions(data)) {
                    tracing::error!("Failed to send ReplayResolutions response: {}", e);
                }
            }))
        })
        .on_replay_data_end({
            let tx = tx.clone();
            Arc::new(Box::new(move |data| {
                if let Err(e) = tx.send(TradingViewResponse::ReplayDataEnd(data)) {
                    tracing::error!("Failed to send ReplayDataEnd response: {}", e);
                }
            }))
        })
        .on_error({
            let tx = tx.clone();
            Arc::new(Box::new(move |(error, values)| {
                if let Err(e) = tx.send(TradingViewResponse::Error(error, values)) {
                    tracing::error!("Failed to send Error response: {}", e);
                }
            }))
        })
        .on_unknown_event({
            let tx = tx.clone();
            Arc::new(Box::new(move |(event, values): (Ustr, Vec<Value>)| {
                if let Err(e) = tx.send(TradingViewResponse::UnknownEvent(event.into(), values)) {
                    tracing::error!("Failed to send UnknownEvent response: {}", e);
                }
            }))
        })
        .build()
}
