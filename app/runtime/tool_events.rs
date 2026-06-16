use foco_tools::{ToolOutputChunk, ToolOutputSink, ToolOutputStream};
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
pub(crate) struct ToolOutputDeltaEvent {
    pub(crate) assistant_message_id: String,
    pub(crate) tool_call_id: String,
    pub(crate) stream: ToolOutputStream,
    pub(crate) delta: String,
}

#[derive(Clone)]
pub(crate) struct ToolOutputDeltaSink {
    pub(crate) assistant_message_id: String,
    pub(crate) tool_call_id: String,
    pub(crate) tx: mpsc::UnboundedSender<ToolOutputDeltaEvent>,
}

impl ToolOutputSink for ToolOutputDeltaSink {
    fn output_chunk(&self, chunk: ToolOutputChunk) {
        if chunk.text.is_empty() {
            return;
        }

        let _ = self.tx.send(ToolOutputDeltaEvent {
            assistant_message_id: self.assistant_message_id.clone(),
            tool_call_id: self.tool_call_id.clone(),
            stream: chunk.stream,
            delta: chunk.text,
        });
    }
}
