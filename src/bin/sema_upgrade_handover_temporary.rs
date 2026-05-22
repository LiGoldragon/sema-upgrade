use sema_upgrade::{ActiveVersion, EndpointState, PrototypeHandover};

fn main() -> Result<(), String> {
    let argument = single_argument()?;
    if argument.trim() != "(RunSpirit010To011)" {
        return Err(format!(
            "expected exactly (RunSpirit010To011), got {}",
            argument.trim()
        ));
    }

    let mut handover = PrototypeHandover::for_spirit_0_1_0_to_0_1_1();
    handover
        .run_atomic_handover()
        .map_err(|error| error.to_string())?;

    if handover.active() != ActiveVersion::Next
        || handover.current().state() != EndpointState::PrivateUpgradeOnly
        || handover.next().state() != EndpointState::Public
    {
        return Err(format!("unexpected handover state: {handover:?}"));
    }

    println!("(SmartHandoverCompleted persona-spirit CurrentPrivateUpgradeOnly NextPublic)");
    Ok(())
}

fn single_argument() -> Result<String, String> {
    let mut arguments = std::env::args().skip(1);
    let Some(argument) = arguments.next() else {
        return Err("expected one NOTA argument".to_string());
    };
    if arguments.next().is_some() {
        return Err("expected exactly one NOTA argument".to_string());
    }
    Ok(argument)
}
