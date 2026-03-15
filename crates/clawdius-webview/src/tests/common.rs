use crate::components::common::{ButtonVariant, ToastType};

#[test]
fn test_button_variant_default() {
    let variant = ButtonVariant::default();
    assert_eq!(variant, ButtonVariant::Primary);
}

#[test]
fn test_button_variant_equality() {
    assert_eq!(ButtonVariant::Primary, ButtonVariant::Primary);
    assert_ne!(ButtonVariant::Primary, ButtonVariant::Secondary);
    assert_ne!(ButtonVariant::Danger, ButtonVariant::Ghost);
}

#[test]
fn test_toast_type_clone() {
    let toast_type = ToastType::Success;
    let cloned = toast_type;
    assert_eq!(toast_type, cloned);
}

#[test]
fn test_toast_type_equality() {
    assert_eq!(ToastType::Success, ToastType::Success);
    assert_ne!(ToastType::Success, ToastType::Error);
    assert_ne!(ToastType::Warning, ToastType::Info);
}
