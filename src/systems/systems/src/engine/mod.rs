use uom::si::f64::*;

use crate::{
    overhead::FirePushButton,
    shared::{EngineCorrectedN2, EngineFirePushButtons},
    simulation::{SimulationElement, SimulationElementVisitor},
};

pub mod leap_engine;

pub trait Engine: EngineCorrectedN2 {
    fn hydraulic_pump_output_speed(&self) -> AngularVelocity;
    fn oil_pressure(&self) -> Pressure;
    fn is_above_minimum_idle(&self) -> bool;
}

pub struct EngineFireOverheadPanel {
    // TODO: Once const generics are available in the dev-env rustc version, we can replace
    // this with an array sized by the const.
    engine_fire_push_buttons: [FirePushButton; 2],
}
impl EngineFireOverheadPanel {
    pub fn new() -> Self {
        Self {
            engine_fire_push_buttons: [FirePushButton::new("ENG1"), FirePushButton::new("ENG2")],
        }
    }
}
impl EngineFirePushButtons for EngineFireOverheadPanel {
    fn is_released(&self, engine_number: usize) -> bool {
        self.engine_fire_push_buttons[engine_number - 1].is_released()
    }
}
impl SimulationElement for EngineFireOverheadPanel {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.engine_fire_push_buttons.iter_mut().for_each(|el| {
            el.accept(visitor);
        });

        visitor.visit(self);
    }
}
impl Default for EngineFireOverheadPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod engine_fire_overhead_panel_tests {
    use crate::simulation::{
        test::{SimulationTestBed, TestBed},
        Write,
    };

    use super::*;

    #[test]
    fn after_construction_fire_push_buttons_are_not_released() {
        let panel = EngineFireOverheadPanel::new();

        assert_eq!(panel.is_released(1), false);
        assert_eq!(panel.is_released(2), false);
    }

    #[test]
    fn fire_push_button_is_released_returns_false_when_not_released() {
        let mut test_bed = SimulationTestBed::from(EngineFireOverheadPanel::new());
        test_bed.write("FIRE_BUTTON_ENG1", false);
        test_bed.run();

        assert_eq!(test_bed.query_element(|e| e.is_released(1)), false);
    }

    #[test]
    fn fire_push_button_is_released_returns_true_when_released() {
        let mut test_bed = SimulationTestBed::from(EngineFireOverheadPanel::new());
        test_bed.write("FIRE_BUTTON_ENG1", true);
        test_bed.run();

        assert_eq!(test_bed.query_element(|e| e.is_released(1)), true);
    }
}
