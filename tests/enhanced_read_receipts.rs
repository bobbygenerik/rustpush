use rustpush::{
    CertifiedContext, should_certify_control_receipt,
};

fn sample_context() -> CertifiedContext {
    CertifiedContext {
        version: 1,
        receipt: vec![0xaa, 0xbb, 0xcc],
        sender: "mailto:peer@example.com".to_string(),
        target: "tel:+15551234567".to_string(),
        token: vec![0x01, 0x02],
        uuid: vec![0x10; 16],
    }
}

#[test]
fn read_receipt_with_certified_context_requests_delivery_certification() {
    let ctx = sample_context();
    // IDS command 102 == Message::Read; IPSW IMReadReceiptProcessingPipelineComponent
    // (0x2346a1fd8, 0x2346a2044, 0x2346a2c40) sends certified delivery if requested.
    let result = should_certify_control_receipt(102, Some(ctx.clone()));
    assert!(result.is_some());
    assert_eq!(result.unwrap().receipt, ctx.receipt);
}

#[test]
fn delivery_receipt_or_missing_context_does_not_certify() {
    let ctx = sample_context();
    assert!(should_certify_control_receipt(101, Some(ctx)).is_none());
    assert!(should_certify_control_receipt(113, None).is_none());
    assert!(should_certify_control_receipt(102, None).is_none());
}
