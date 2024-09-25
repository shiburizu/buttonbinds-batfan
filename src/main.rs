use enigo::{
    Direction,
    Direction::{Press, Release},
    Enigo, Key, Keyboard, Settings,
};
use std::collections::HashMap;
use std::io;
use std::sync::mpsc;
use std::thread;
extern crate sdl2;
use sdl2::controller::{Axis, Button, GameController};
use sdl2::event::Event;

#[derive(Eq, Hash, PartialEq)]
enum ControllerInput {
    Analog(Axis),
    Digital(Button),
}

fn bind(
    bindings: &mut [HashMap<u32, HashMap<ControllerInput, Key>>; 2],
    p_idx: usize,
    c_idx: u32,
    input: ControllerInput,
    k: Key,
) {
    // could probably do this easier with try_insert if it ever gets added
    match bindings[p_idx].get_mut(&c_idx) {
        Some(controller_bindings) => {
            controller_bindings.insert(input, k);
        }
        None => {
            bindings[p_idx].insert(c_idx, HashMap::new());
            let controller_bindings = bindings[p_idx].get_mut(&c_idx).unwrap();
            controller_bindings.insert(input, k);
        }
    }
}

fn press(
    bindings: &[HashMap<u32, HashMap<ControllerInput, Key>>; 2],
    enigo: &mut Enigo,
    c_idx: u32,
    input: ControllerInput,
    action: Direction,
) {
    for binding in bindings {
        match binding.get(&c_idx) {
            Some(controller_bindings) => match controller_bindings.get(&input) {
                Some(key) => {
                    let _ = enigo.key(*key, action);
                }
                _ => (),
            },
            _ => (),
        }
    }
}

fn main() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    sdl2::hint::set("SDL_JOYSTICK_THREAD", "1");
    sdl2::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1"); // might not be necessary

    let sdl_context = sdl2::init()?;
    let game_controller_subsystem = sdl_context.game_controller()?;
    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut controllers: HashMap<u32, GameController> = HashMap::new();
    let mut controller_analog_states: HashMap<u32, [bool; 4]> = HashMap::new();
    let mut bindings: [HashMap<u32, HashMap<ControllerInput, Key>>; 2] =
        [HashMap::new(), HashMap::new()];

    #[cfg(target_os = "linux")]
    let directions: [HashMap<&str, Key>; 2] = [
        HashMap::from([
            ("Up", Key::Unicode('t')),
            ("Down", Key::Unicode('b')),
            ("Left", Key::Unicode('f')),
            ("Right", Key::Unicode('h')),
        ]),
        HashMap::from([
            // should probably combine these into one type but quick and hacky is the way for right now
            ("Up", Key::Other(0xffb8)),
            ("Down", Key::Other(0xffb2)),
            ("Left", Key::Other(0xffb4)),
            ("Right", Key::Other(0xffb6)),
        ]),
    ];

    #[cfg(target_os = "windows")]
    let directions: [HashMap<&str, Key>; 2] = [
        HashMap::from([
            ("Up", Key::Unicode('t')),
            ("Down", Key::Unicode('b')),
            ("Left", Key::Unicode('f')),
            ("Right", Key::Unicode('h')),
        ]),
        HashMap::from([
            ("Up", Key::Other(0x68)),
            ("Down", Key::Other(0x62)),
            ("Left", Key::Other(0x64)),
            ("Right", Key::Other(0x66)),
        ]),
    ];

    let actions: [[(&str, Key); 7]; 2] = [
        [
            ("Punch", Key::Unicode('a')),
            ("Kick", Key::Unicode('s')),
            ("Slash", Key::Unicode('d')),
            ("Heavy Slash", Key::Unicode('q')),
            ("Original Action", Key::Unicode('w')),
            ("Special Action", Key::Unicode('e')),
            ("Pause", Key::Escape),
        ],
        [
            ("Punch", Key::Unicode('j')),
            ("Kick", Key::Unicode('k')),
            ("Slash", Key::Unicode('l')),
            ("Heavy Slash", Key::Unicode('i')),
            ("Original Action", Key::Unicode('o')),
            ("Special Action", Key::Unicode('p')),
            ("Pause", Key::Escape),
        ],
    ];

    let (tx, rx) = mpsc::channel();
    println!("Please enter either 1 or 2.");
    thread::spawn(move || loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        match input.trim() {
            "1" => tx.send(0).unwrap(),
            "2" => tx.send(1).unwrap(),
            _ => println!("Please enter either 1 or 2."),
        }
    });

    loop {
        match rx.try_recv() {
            Ok(index) => {
                println!("Binding buttons for Player {}. Press the buttons you would like for the corresponding actions:", index + 1);
                bindings[index].drain();
                let mut controller_id: u32 = 0; // should probably be option again, too lazy to change it back
                for (action, key) in actions[index] {
                    println!("{}:", action);
                    'waiting_input: loop {
                        // swap this to next_event_blocking w/ filters later if possible
                        for event in event_pump.poll_iter() {
                            match event {
                                Event::ControllerButtonDown {
                                    timestamp: _,
                                    which,
                                    button,
                                } => {
                                    controller_id = which;
                                    bind(
                                        &mut bindings,
                                        index,
                                        which,
                                        ControllerInput::Digital(button),
                                        key,
                                    );
                                    break 'waiting_input;
                                }
                                Event::ControllerDeviceAdded {
                                    timestamp: _,
                                    which,
                                } => match game_controller_subsystem.open(which) {
                                    Ok(c) => {
                                        controller_analog_states
                                            .insert(c.instance_id(), [false; 4]);
                                        controllers.insert(c.instance_id(), c);
                                    }
                                    Err(_) => (),
                                },
                                Event::ControllerDeviceRemoved {
                                    timestamp: _,
                                    which,
                                } => {
                                    controllers.remove(&which);
                                    controller_analog_states.remove(&which);
                                }
                                Event::ControllerAxisMotion {
                                    timestamp: _,
                                    which,
                                    axis,
                                    value,
                                } => match axis {
                                    Axis::TriggerRight | Axis::TriggerLeft => {
                                        let state_idx = match axis {
                                            Axis::TriggerRight => 0,
                                            Axis::TriggerLeft => 1,
                                            _ => panic!(),
                                        };
                                        let old_state = controller_analog_states
                                            .get(&which)
                                            .unwrap()[state_idx];
                                        let new_state = value.unsigned_abs() > i16::MAX as u16 / 2;
                                        if old_state != new_state {
                                            controller_analog_states.get_mut(&which).unwrap()
                                                [state_idx] = new_state;
                                            if new_state {
                                                bind(
                                                    &mut bindings,
                                                    index,
                                                    which,
                                                    ControllerInput::Analog(axis),
                                                    key,
                                                );
                                                break 'waiting_input;
                                            }
                                        }
                                    }
                                    _ => (),
                                },
                                Event::Quit { .. } => return Ok(()),
                                _ => (),
                            }
                        }
                    }
                }

                // should probably just initialize bindings map with these already in it once they have controller select
                // let controller_bindings = bindings[index].get_mut(&controller_id).unwrap();
                bind(
                    &mut bindings,
                    index,
                    controller_id,
                    ControllerInput::Digital(Button::DPadUp),
                    *directions[index].get("Up").unwrap(),
                );
                bind(
                    &mut bindings,
                    index,
                    controller_id,
                    ControllerInput::Digital(Button::DPadDown),
                    *directions[index].get("Down").unwrap(),
                );
                bind(
                    &mut bindings,
                    index,
                    controller_id,
                    ControllerInput::Digital(Button::DPadLeft),
                    *directions[index].get("Left").unwrap(),
                );
                bind(
                    &mut bindings,
                    index,
                    controller_id,
                    ControllerInput::Digital(Button::DPadRight),
                    *directions[index].get("Right").unwrap(),
                );
                println!("Finished with binding, please double check bindings in game.");
                println!("Please enter either 1 or 2.");
            }
            Err(_) => (),
        }

        for event in event_pump.poll_iter() {
            match event {
                Event::ControllerButtonDown {
                    timestamp: _,
                    which,
                    button,
                } => press(
                    &bindings,
                    &mut enigo,
                    which,
                    ControllerInput::Digital(button),
                    Press,
                ),

                Event::ControllerButtonUp {
                    timestamp: _,
                    which,
                    button,
                } => press(
                    &bindings,
                    &mut enigo,
                    which,
                    ControllerInput::Digital(button),
                    Release,
                ),

                Event::ControllerAxisMotion {
                    timestamp: _,
                    which,
                    axis,
                    value,
                } => match axis {
                    Axis::TriggerRight | Axis::TriggerLeft => {
                        let state_idx = match axis {
                            Axis::TriggerRight => 0,
                            Axis::TriggerLeft => 1,
                            _ => panic!(),
                        };
                        let old_state = controller_analog_states.get(&which).unwrap()[state_idx];
                        let new_state = value.unsigned_abs() > i16::MAX as u16 / 2;
                        if old_state != new_state {
                            controller_analog_states.get_mut(&which).unwrap()[state_idx] =
                                new_state;
                            if new_state {
                                press(
                                    &bindings,
                                    &mut enigo,
                                    which,
                                    ControllerInput::Analog(axis),
                                    Press,
                                );
                            } else {
                                press(
                                    &bindings,
                                    &mut enigo,
                                    which,
                                    ControllerInput::Analog(axis),
                                    Release,
                                );
                            }
                        }
                    }
                    Axis::LeftX | Axis::LeftY => {
                        let (state_idx, dpad) = match (axis, value > 0) {
                            (Axis::LeftX, true) => (2, Button::DPadRight),
                            (Axis::LeftX, false) => (2, Button::DPadLeft),
                            (Axis::LeftY, true) => (3, Button::DPadDown),
                            (Axis::LeftY, false) => (3, Button::DPadUp),
                            _ => panic!(),
                        };
                        let old_state = controller_analog_states.get(&which).unwrap()[state_idx];
                        let new_state = value.unsigned_abs() > i16::MAX as u16 / 2;

                        if old_state != new_state {
                            controller_analog_states.get_mut(&which).unwrap()[state_idx] =
                                new_state;
                            if new_state {
                                press(
                                    &bindings,
                                    &mut enigo,
                                    which,
                                    ControllerInput::Digital(dpad),
                                    Press,
                                );
                            } else {
                                // should be able to simplify with changing dpad var above, is fine for now
                                if state_idx == 2 {
                                    press(
                                        &bindings,
                                        &mut enigo,
                                        which,
                                        ControllerInput::Digital(Button::DPadRight),
                                        Release,
                                    );
                                    press(
                                        &bindings,
                                        &mut enigo,
                                        which,
                                        ControllerInput::Digital(Button::DPadLeft),
                                        Release,
                                    );
                                } else {
                                    press(
                                        &bindings,
                                        &mut enigo,
                                        which,
                                        ControllerInput::Digital(Button::DPadDown),
                                        Release,
                                    );
                                    press(
                                        &bindings,
                                        &mut enigo,
                                        which,
                                        ControllerInput::Digital(Button::DPadUp),
                                        Release,
                                    );
                                }
                            }
                        }
                    }
                    _ => (),
                },
                Event::ControllerDeviceAdded {
                    timestamp: _,
                    which,
                } => match game_controller_subsystem.open(which) {
                    Ok(c) => {
                        controller_analog_states.insert(c.instance_id(), [false; 4]);
                        controllers.insert(c.instance_id(), c);
                    }
                    Err(_) => (),
                },
                Event::ControllerDeviceRemoved {
                    timestamp: _,
                    which,
                } => {
                    controllers.remove(&which);
                    controller_analog_states.remove(&which);
                }
                Event::Quit { .. } => return Ok(()),
                _ => (),
            }
        }
    }
}
