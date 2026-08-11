use dicar_app_core::{validate_subscription, Endpoint, SerialHardwareProfile};

fn serial(profile: SerialHardwareProfile, baud_rate: u32) -> Endpoint {
    Endpoint::Serial {
        port_name: "COM12".into(),
        baud_rate,
        hardware_profile: profile,
    }
}

#[test]
fn hc05_uses_a_conservative_budget_and_a_stricter_9600_baud_mode() {
    let normal = serial(SerialHardwareProfile::Hc05BluetoothSpp, 115_200);
    assert!(validate_subscription(&normal, 4, 50).is_ok());
    assert_eq!(
        validate_subscription(&normal, 5, 50)
            .unwrap_err()
            .to_string(),
        "HC-05 当前链路最多 4 个通道"
    );
    assert_eq!(
        validate_subscription(&normal, 4, 100)
            .unwrap_err()
            .to_string(),
        "HC-05 当前链路最高 50 Hz"
    );

    let low_rate = serial(SerialHardwareProfile::Hc05BluetoothSpp, 9_600);
    assert!(validate_subscription(&low_rate, 2, 10).is_ok());
    assert!(validate_subscription(&low_rate, 3, 10).is_err());
    assert!(validate_subscription(&low_rate, 2, 11).is_err());
}

#[test]
fn nano_uart_and_simulator_keep_their_high_rate_budget() {
    let nano = serial(SerialHardwareProfile::NanoUartWl, 460_800);
    let simulator = Endpoint::Simulator {
        address: "127.0.0.1:7100".parse().unwrap(),
    };
    assert!(validate_subscription(&nano, 8, 500).is_ok());
    assert!(validate_subscription(&simulator, 8, 500).is_ok());
}
