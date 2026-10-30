use super::*;

#[test]
fn selection_validation_rejects_duplicates_invalid_imitators_and_overflow() {
    let ice = CardSelection::Plant(PlantKind::IceShroom);
    assert!(matches!(
        validate_card_selection(&[ice, ice]),
        Err(CoreLogicError::DuplicateCardSelection(_))
    ));
    assert!(matches!(
        validate_card_selection(&[
            CardSelection::Imitator(PlantKind::IceShroom),
            CardSelection::Imitator(PlantKind::DoomShroom)
        ]),
        Err(CoreLogicError::MultipleImitatorCards)
    ));
    assert!(matches!(
        validate_card_selection(&[CardSelection::Plant(PlantKind::Imitator)]),
        Err(CoreLogicError::InvalidCardSelection(_))
    ));
    assert!(matches!(
        validate_card_selection(&[ice; MAX_SEED_SLOTS + 1]),
        Err(CoreLogicError::TooManyCards { .. })
    ));
}

#[cfg(all(test, feature = "backend-tests"))]
#[test]
fn native_action_rejection_preserves_cause_without_reporting_or_backend_access() {
    let error = CardLogicError::Backend(crate::access::operation_error(
        rsvz_current::CurrentBackendError::OperationRejected("add plant"),
    ));
    let error = card_error_at(CardSelection::Plant(PlantKind::IceShroom), 3, 9, &error);
    assert_eq!(error.kind(), CardErrorKind::ActionRejected);
    let error = RuntimeError::from(error);
    assert!(error.find_source::<CardError>().is_some());
    assert!(error.find_source::<rsvz_current::CurrentBackendError>().is_some());
}
