//! Isolated transport contract tests; no sockets or real credentials.
use joyword_agent::{
    AgentRuntime, CancelFlag, CompletionRequest, HttpProvider, Outbound, Provider, ProviderError,
    Secret, Transport,
};

struct CancelDuringExchange<'a> {
    cancel: &'a CancelFlag,
    result: Result<Vec<u8>, ProviderError>,
}
impl Transport for CancelDuringExchange<'_> {
    fn exchange(&self, call: &Outbound<'_>) -> Result<Vec<u8>, ProviderError> {
        assert_eq!(call.authorization.unwrap().expose(), "synthetic-test-token");
        self.cancel.cancel();
        self.result.clone()
    }
}

#[test]
fn cancellation_during_legacy_transport_discards_success_and_error() {
    for result in [
        Ok(b"late response".to_vec()),
        Ok(vec![255]),
        Err(ProviderError::Offline),
    ] {
        let cancel = CancelFlag::new();
        let provider = HttpProvider::new(
            "local",
            "http://localhost/test",
            CancelDuringExchange {
                cancel: &cancel,
                result,
            },
        )
        .unwrap();
        let secret = Secret::new("synthetic-test-token").unwrap();
        let request = CompletionRequest {
            prompt: "synthetic prompt",
            generation: AgentRuntime::boot_disabled().generation().unwrap(),
            cancel: &cancel,
            credential: Some(&secret),
        };
        assert_eq!(provider.complete(&request), Err(ProviderError::Cancelled));
        assert_eq!(provider.calls(), 1);
        assert_eq!(provider.complete(&request), Err(ProviderError::Cancelled));
        assert_eq!(provider.calls(), 1);
    }
}

struct CooperativeTransport;
impl Transport for CooperativeTransport {
    fn exchange(&self, _: &Outbound<'_>) -> Result<Vec<u8>, ProviderError> {
        panic!("provider must use the cancellable entry point")
    }

    fn exchange_cancellable(
        &self,
        _: &Outbound<'_>,
        cancel: &CancelFlag,
    ) -> Result<Vec<u8>, ProviderError> {
        cancel.cancel();
        // A transport may still produce a result at the cancellation boundary.
        Ok(b"late cooperative result".to_vec())
    }
}

#[test]
fn provider_passes_its_cancel_flag_to_cooperative_transport() {
    let cancel = CancelFlag::new();
    let provider =
        HttpProvider::new("local", "http://localhost/test", CooperativeTransport).unwrap();
    let request = CompletionRequest {
        prompt: "synthetic prompt",
        generation: AgentRuntime::boot_disabled().generation().unwrap(),
        cancel: &cancel,
        credential: None,
    };
    assert_eq!(provider.complete(&request), Err(ProviderError::Cancelled));
    assert!(cancel.is_cancelled());
}

#[test]
fn default_transport_entry_refuses_pre_cancelled_request() {
    let cancel = CancelFlag::new();
    cancel.cancel();
    let transport = joyword_agent::RecordingTransport::new();
    let call = Outbound {
        url: "http://localhost/test",
        body: "synthetic",
        authorization: None,
    };
    assert_eq!(
        transport.exchange_cancellable(&call, &cancel),
        Err(ProviderError::Cancelled)
    );
    assert!(transport.is_empty());
}
