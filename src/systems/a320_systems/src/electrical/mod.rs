mod alternating_current;
mod direct_current;
mod galley;

use self::{
    alternating_current::A320AlternatingCurrentElectrical,
    direct_current::A320DirectCurrentElectrical,
    galley::{MainGalley, SecondaryGalley},
};
pub(super) use direct_current::APU_START_MOTOR_BUS_TYPE;
#[cfg(test)]
use systems::electrical::Battery;
use systems::{
    electrical::{
        AlternatingCurrentElectricalSystem, BatteryPushButtons,
        ElectricalElementIdentifierProvider, Electricity, EmergencyElectrical, EmergencyGenerator,
        EngineGeneratorPushButtons, ExternalPowerSource, StaticInverter, TransformerRectifier,
    },
    overhead::{
        AutoOffFaultPushButton, FaultIndication, FaultReleasePushButton, MomentaryPushButton,
        NormalAltnFaultPushButton, OnOffAvailablePushButton, OnOffFaultPushButton,
    },
    shared::{
        ApuMaster, ApuStart, AuxiliaryPowerUnitElectrical, EmergencyElectricalRatPushButton,
        EmergencyElectricalState, EngineCorrectedN2, EngineFirePushButtons, LandingGearPosition,
        RamAirTurbineHydraulicLoopPressurised,
    },
    simulation::{
        SimulationElement, SimulationElementVisitor, SimulatorWriter, UpdateContext, Write,
    },
};

pub(super) struct A320Electrical {
    alternating_current: A320AlternatingCurrentElectrical,
    direct_current: A320DirectCurrentElectrical,
    main_galley: MainGalley,
    secondary_galley: SecondaryGalley,
    emergency_elec: EmergencyElectrical,
    emergency_gen: EmergencyGenerator,
}
impl A320Electrical {
    pub fn new(
        identifier_provider: &mut impl ElectricalElementIdentifierProvider,
    ) -> A320Electrical {
        A320Electrical {
            alternating_current: A320AlternatingCurrentElectrical::new(identifier_provider),
            direct_current: A320DirectCurrentElectrical::new(identifier_provider),
            main_galley: MainGalley::new(),
            secondary_galley: SecondaryGalley::new(),
            emergency_elec: EmergencyElectrical::new(),
            emergency_gen: EmergencyGenerator::new(identifier_provider),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        context: &UpdateContext,
        electricity: &mut Electricity,
        ext_pwr: &ExternalPowerSource,
        overhead: &A320ElectricalOverheadPanel,
        emergency_overhead: &A320EmergencyElectricalOverheadPanel,
        apu: &mut impl AuxiliaryPowerUnitElectrical,
        apu_overhead: &(impl ApuMaster + ApuStart),
        engine_fire_push_buttons: &impl EngineFirePushButtons,
        engines: [&impl EngineCorrectedN2; 2],
        hydraulic: &impl RamAirTurbineHydraulicLoopPressurised,
        landing_gear: &impl LandingGearPosition,
    ) {
        self.alternating_current.update_main_power_sources(
            context,
            electricity,
            ext_pwr,
            overhead,
            emergency_overhead,
            apu,
            engine_fire_push_buttons,
            engines,
        );

        self.emergency_elec
            .update(context, electricity, &self.alternating_current);

        if (self.emergency_elec.is_active()) || emergency_overhead.rat_and_emer_gen_man_on() {
            self.emergency_gen.start();
        }

        self.emergency_gen.update(context, hydraulic);

        self.alternating_current.update(
            context,
            electricity,
            ext_pwr,
            overhead,
            &self.emergency_gen,
        );

        self.direct_current.update(
            context,
            electricity,
            overhead,
            &self.alternating_current,
            &self.emergency_elec,
            &self.emergency_gen,
            apu,
            apu_overhead,
            landing_gear,
        );

        self.alternating_current.update_after_direct_current(
            context,
            electricity,
            &self.emergency_gen,
            &self.direct_current,
        );

        self.main_galley
            .update(context, electricity, &self.alternating_current, overhead);
        self.secondary_galley
            .update(electricity, &self.alternating_current, overhead);

        self.debug_assert_invariants();
    }

    fn emergency_generator_contactor_is_closed(&self) -> bool {
        self.alternating_current
            .emergency_generator_contactor_is_closed()
    }

    fn ac_ess_bus_is_powered(&self, electricity: &Electricity) -> bool {
        self.alternating_current.ac_ess_bus_is_powered(electricity)
    }

    fn galley_is_shed(&self) -> bool {
        self.main_galley.is_shed() || self.secondary_galley.is_shed()
    }

    fn debug_assert_invariants(&self) {
        self.alternating_current.debug_assert_invariants();
        self.direct_current.debug_assert_invariants();
    }

    #[cfg(test)]
    fn fail_tr_1(&mut self) {
        self.alternating_current.fail_tr_1();
    }

    #[cfg(test)]
    fn fail_tr_2(&mut self) {
        self.alternating_current.fail_tr_2();
    }

    #[cfg(test)]
    fn attempt_emergency_gen_start(&mut self) {
        self.emergency_gen.start();
    }

    #[cfg(test)]
    fn tr_1(&self) -> &TransformerRectifier {
        self.alternating_current.tr_1()
    }

    #[cfg(test)]
    fn tr_2(&self) -> &TransformerRectifier {
        self.alternating_current.tr_2()
    }

    #[cfg(test)]
    fn tr_ess(&self) -> &TransformerRectifier {
        self.alternating_current.tr_ess()
    }

    #[cfg(test)]
    fn battery_1(&self) -> &Battery {
        self.direct_current.battery_1()
    }

    #[cfg(test)]
    fn battery_2(&self) -> &Battery {
        self.direct_current.battery_2()
    }

    #[cfg(test)]
    pub fn empty_battery_1(&mut self) {
        self.direct_current.empty_battery_1();
    }

    #[cfg(test)]
    pub fn empty_battery_2(&mut self) {
        self.direct_current.empty_battery_2();
    }

    pub fn gen_contactor_open(&self, number: usize) -> bool {
        self.alternating_current.gen_contactor_open(number)
    }

    pub fn in_emergency_elec(&self) -> bool {
        self.emergency_elec.is_active()
    }
}
impl SimulationElement for A320Electrical {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.alternating_current.accept(visitor);
        self.direct_current.accept(visitor);
        self.emergency_gen.accept(visitor);

        visitor.visit(self);
    }

    fn write(&self, writer: &mut SimulatorWriter) {
        writer.write("ELEC_GALLEY_IS_SHED", self.galley_is_shed())
    }
}
impl EmergencyElectricalState for A320Electrical {
    fn is_in_emergency_elec(&self) -> bool {
        self.in_emergency_elec()
    }
}

trait A320DirectCurrentElectricalSystem {
    fn static_inverter(&self) -> &StaticInverter;
}

trait A320AlternatingCurrentElectricalSystem: AlternatingCurrentElectricalSystem {
    fn ac_bus_2_powered(&self, electricity: &Electricity) -> bool;
    fn tr_1_and_2_available(&self, electricity: &Electricity) -> bool;
    fn tr_1(&self) -> &TransformerRectifier;
    fn tr_2(&self) -> &TransformerRectifier;
    fn tr_ess(&self) -> &TransformerRectifier;
}

pub(super) struct A320ElectricalOverheadPanel {
    batteries: [AutoOffFaultPushButton; 2],
    idgs: [FaultReleasePushButton; 2],
    generators: [OnOffFaultPushButton; 2],
    apu_gen: OnOffFaultPushButton,
    bus_tie: AutoOffFaultPushButton,
    ac_ess_feed: NormalAltnFaultPushButton,
    galy_and_cab: AutoOffFaultPushButton,
    ext_pwr: OnOffAvailablePushButton,
    commercial: OnOffFaultPushButton,
}
impl A320ElectricalOverheadPanel {
    pub fn new() -> A320ElectricalOverheadPanel {
        A320ElectricalOverheadPanel {
            batteries: [
                AutoOffFaultPushButton::new_auto("ELEC_BAT_1"),
                AutoOffFaultPushButton::new_auto("ELEC_BAT_2"),
            ],
            idgs: [
                FaultReleasePushButton::new_in("ELEC_IDG_1"),
                FaultReleasePushButton::new_in("ELEC_IDG_2"),
            ],
            generators: [
                OnOffFaultPushButton::new_on("ELEC_ENG_GEN_1"),
                OnOffFaultPushButton::new_on("ELEC_ENG_GEN_2"),
            ],
            apu_gen: OnOffFaultPushButton::new_on("ELEC_APU_GEN"),
            bus_tie: AutoOffFaultPushButton::new_auto("ELEC_BUS_TIE"),
            ac_ess_feed: NormalAltnFaultPushButton::new_normal("ELEC_AC_ESS_FEED"),
            galy_and_cab: AutoOffFaultPushButton::new_auto("ELEC_GALY_AND_CAB"),
            ext_pwr: OnOffAvailablePushButton::new_off("ELEC_EXT_PWR"),
            commercial: OnOffFaultPushButton::new_on("ELEC_COMMERCIAL"),
        }
    }

    pub fn update_after_electrical(
        &mut self,
        electrical: &A320Electrical,
        electricity: &Electricity,
    ) {
        self.ac_ess_feed
            .set_fault(!electrical.ac_ess_bus_is_powered(electricity));

        self.generators
            .iter_mut()
            .enumerate()
            .for_each(|(index, gen)| {
                gen.set_fault(electrical.gen_contactor_open(index + 1) && gen.is_on());
            });
    }

    fn generator_is_on(&self, number: usize) -> bool {
        self.generators[number - 1].is_on()
    }

    pub fn external_power_is_available(&self) -> bool {
        self.ext_pwr.is_available()
    }

    pub fn external_power_is_on(&self) -> bool {
        self.ext_pwr.is_on()
    }

    pub fn apu_generator_is_on(&self) -> bool {
        self.apu_gen.is_on()
    }

    fn bus_tie_is_auto(&self) -> bool {
        self.bus_tie.is_auto()
    }

    fn ac_ess_feed_is_normal(&self) -> bool {
        self.ac_ess_feed.is_normal()
    }

    fn ac_ess_feed_is_altn(&self) -> bool {
        self.ac_ess_feed.is_altn()
    }

    fn commercial_is_off(&self) -> bool {
        self.commercial.is_off()
    }

    fn galy_and_cab_is_off(&self) -> bool {
        self.galy_and_cab.is_off()
    }
}
impl EngineGeneratorPushButtons for A320ElectricalOverheadPanel {
    fn engine_gen_push_button_is_on(&self, number: usize) -> bool {
        self.generators[number - 1].is_on()
    }

    fn idg_push_button_is_released(&self, number: usize) -> bool {
        self.idgs[number - 1].is_released()
    }
}
impl BatteryPushButtons for A320ElectricalOverheadPanel {
    fn bat_is_auto(&self, number: usize) -> bool {
        self.batteries[number - 1].is_auto()
    }
}
impl SimulationElement for A320ElectricalOverheadPanel {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.batteries.iter_mut().for_each(|bat| {
            bat.accept(visitor);
        });
        self.idgs.iter_mut().for_each(|idg| {
            idg.accept(visitor);
        });
        self.generators.iter_mut().for_each(|gen| {
            gen.accept(visitor);
        });
        self.apu_gen.accept(visitor);
        self.bus_tie.accept(visitor);
        self.ac_ess_feed.accept(visitor);
        self.galy_and_cab.accept(visitor);
        self.ext_pwr.accept(visitor);
        self.commercial.accept(visitor);

        visitor.visit(self);
    }
}

pub(super) struct A320EmergencyElectricalOverheadPanel {
    // The GEN 1 line fault represents the SMOKE light illumination state.
    gen_1_line: OnOffFaultPushButton,
    rat_and_emergency_gen_fault: FaultIndication,
    rat_and_emer_gen_man_on: MomentaryPushButton,
}
impl A320EmergencyElectricalOverheadPanel {
    pub fn new() -> Self {
        Self {
            gen_1_line: OnOffFaultPushButton::new_on("EMER_ELEC_GEN_1_LINE"),
            rat_and_emergency_gen_fault: FaultIndication::new("EMER_ELEC_RAT_AND_EMER_GEN"),
            rat_and_emer_gen_man_on: MomentaryPushButton::new("EMER_ELEC_RAT_AND_EMER_GEN"),
        }
    }

    pub fn update_after_electrical(
        &mut self,
        context: &UpdateContext,
        electrical: &A320Electrical,
    ) {
        self.rat_and_emergency_gen_fault.set_fault(
            electrical.in_emergency_elec()
                && !electrical.emergency_generator_contactor_is_closed()
                && !context.is_on_ground(),
        );
    }

    fn generator_1_line_is_on(&self) -> bool {
        self.gen_1_line.is_on()
    }

    fn rat_and_emer_gen_man_on(&self) -> bool {
        self.rat_and_emer_gen_man_on.is_pressed()
    }
}
impl SimulationElement for A320EmergencyElectricalOverheadPanel {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.gen_1_line.accept(visitor);
        self.rat_and_emergency_gen_fault.accept(visitor);
        self.rat_and_emer_gen_man_on.accept(visitor);

        visitor.visit(self);
    }
}
impl EmergencyElectricalRatPushButton for A320EmergencyElectricalOverheadPanel {
    fn is_pressed(&self) -> bool {
        self.rat_and_emer_gen_man_on()
    }
}
#[cfg(test)]
mod a320_electrical {
    use super::*;
    use systems::simulation::test::{ElementCtorFn, SimulationTestBed, TestBed};

    #[test]
    fn writes_its_state() {
        let mut test_bed = SimulationTestBed::from(ElementCtorFn(A320Electrical::new));

        test_bed.run();

        assert!(test_bed.contains_key("ELEC_GALLEY_IS_SHED"));
    }
}

#[cfg(test)]
mod a320_electrical_circuit_tests {
    use super::{alternating_current::A320AcEssFeedContactors, *};
    use rstest::rstest;
    use std::{cell::Ref, time::Duration};
    use systems::{
        electrical::{
            ElectricalElement, ElectricalElementIdentifier, Electricity, ElectricitySource,
            ExternalPowerSource, Potential,
            INTEGRATED_DRIVE_GENERATOR_STABILIZATION_TIME_IN_MILLISECONDS,
        },
        shared::{
            ApuAvailable, ContactorSignal, ControllerSignal, ElectricalBusType, ElectricalBuses,
            PotentialOrigin,
        },
        simulation::{
            test::{SimulationTestBed, TestBed},
            Aircraft, Read,
        },
    };
    use uom::si::f64::*;
    use uom::si::{electric_potential::volt, length::foot, ratio::percent, velocity::knot};

    #[test]
    fn everything_off_batteries_empty() {
        let test_bed = test_bed_with()
            .bat_off(1)
            .empty_battery_1()
            .bat_off(2)
            .empty_battery_2()
            .and()
            .airspeed(Velocity::new::<knot>(0.))
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(test_bed.ac_ess_bus_output().is_unpowered());
        assert!(test_bed.ac_ess_shed_bus_output().is_unpowered());
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed.dc_bat_bus_output().is_unpowered());
        assert!(test_bed.dc_ess_bus_output().is_unpowered());
        assert!(test_bed.dc_ess_shed_bus_output().is_unpowered());
        assert!(test_bed.hot_bus_output(1).is_unpowered());
        assert!(test_bed.hot_bus_output(2).is_unpowered());
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    #[test]
    fn everything_off() {
        let test_bed = test_bed_with()
            .bat_off(1)
            .bat_off(2)
            .and()
            .airspeed(Velocity::new::<knot>(0.))
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(test_bed.ac_ess_bus_output().is_unpowered());
        assert!(test_bed.ac_ess_shed_bus_output().is_unpowered());
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed.dc_bat_bus_output().is_unpowered());
        assert!(test_bed.dc_ess_bus_output().is_unpowered());
        assert!(test_bed.dc_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_norm_conf() {
        let test_bed = test_bed_with().running_engines().run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .tr_1_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .tr_2_input()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed
            .dc_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_bat_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_only_gen_1_available() {
        let test_bed = test_bed_with().running_engine(1).run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .tr_1_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .tr_2_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed
            .dc_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_bat_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_only_gen_2_available() {
        let test_bed = test_bed_with().running_engine(2).run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .tr_1_input()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .tr_2_input()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed
            .dc_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_bat_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_only_apu_gen_available() {
        let test_bed = test_bed_with().running_apu().run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed
            .tr_1_input()
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed
            .tr_2_input()
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed
            .dc_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_bat_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    /// # Source
    /// Derived from A320 manual electrical distribution table
    /// (doesn't list external power, but we'll assume it's the same as other generators).
    #[test]
    fn distribution_table_only_external_power_available_and_on() {
        let test_bed = test_bed_with()
            .connected_external_power()
            .airspeed(Velocity::new::<knot>(0.))
            .on_the_ground()
            .and()
            .ext_pwr_on()
            .run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::External));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::External));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::External));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::External));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::External));
        assert!(test_bed.tr_1_input().is_single(PotentialOrigin::External));
        assert!(test_bed.tr_2_input().is_single(PotentialOrigin::External));
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed
            .dc_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_bat_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_emergency_config_before_emergency_gen_available() {
        let test_bed = test_bed().run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(test_bed.ac_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .static_inverter_input()
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .ac_stat_inv_bus_output()
            .is_single(PotentialOrigin::StaticInverter));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::StaticInverter));
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed.dc_bat_bus_output().is_unpowered());
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_emergency_config_after_emergency_gen_available() {
        let test_bed = test_bed_with().running_emergency_generator().run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EmergencyGenerator));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EmergencyGenerator));
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed
            .tr_ess_input()
            .is_single(PotentialOrigin::EmergencyGenerator));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed.dc_bat_bus_output().is_unpowered());
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_tr_1_fault() {
        let test_bed = test_bed_with().running_engines().and().failed_tr_1().run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .tr_1_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .tr_2_input()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .tr_ess_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .dc_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_bat_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_tr_2_fault() {
        let test_bed = test_bed_with().running_engines().and().failed_tr_2().run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed
            .tr_1_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed
            .tr_ess_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .dc_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_bat_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_tr_1_and_2_fault() {
        let test_bed = test_bed_with()
            .running_engines()
            .failed_tr_1()
            .and()
            .failed_tr_2()
            .run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed
            .tr_1_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed
            .tr_ess_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed.dc_bat_bus_output().is_unpowered());
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_on_ground_bat_and_emergency_gen_only_speed_above_100_knots() {
        let test_bed = test_bed_with()
            .running_emergency_generator()
            .airspeed(Velocity::new::<knot>(101.))
            .and()
            .on_the_ground()
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EmergencyGenerator));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EmergencyGenerator));
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed
            .tr_ess_input()
            .is_single(PotentialOrigin::EmergencyGenerator));
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed.dc_bat_bus_output().is_unpowered());
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_on_ground_bat_only_rat_stall_or_speed_between_50_to_100_knots() {
        let test_bed = test_bed_with()
            .running_emergency_generator()
            .airspeed(Velocity::new::<knot>(50.0))
            .and()
            .on_the_ground()
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::StaticInverter));
        assert!(test_bed.ac_ess_shed_bus_output().is_unpowered());
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed
            .static_inverter_input()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .ac_stat_inv_bus_output()
            .is_single(PotentialOrigin::StaticInverter));
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed
            .dc_bat_bus_output()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .hot_bus_output(1)
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_on_ground_bat_only_speed_less_than_50_knots() {
        let test_bed = test_bed_with()
            .running_emergency_generator()
            .airspeed(Velocity::new::<knot>(49.9))
            .and()
            .on_the_ground()
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(
            test_bed.ac_ess_bus_output().is_unpowered(),
            "AC ESS BUS shouldn't be powered below 50 knots when on batteries only."
        );
        assert!(test_bed.ac_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .static_inverter_input()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .ac_stat_inv_bus_output()
            .is_single(PotentialOrigin::StaticInverter));
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed
            .dc_bat_bus_output()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .hot_bus_output(1)
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    #[test]
    fn distribution_table_only_external_power_available_and_off() {
        let test_bed = test_bed_with()
            .connected_external_power()
            .airspeed(Velocity::new::<knot>(0.))
            .and()
            .on_the_ground()
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(test_bed.ac_ess_bus_output().is_unpowered());
        assert!(test_bed.ac_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .static_inverter_input()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .ac_stat_inv_bus_output()
            .is_single(PotentialOrigin::StaticInverter));
        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::External));
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_single(PotentialOrigin::External));
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed
            .dc_bat_bus_output()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .hot_bus_output(1)
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    #[rstest]
    #[case(1, 2)]
    #[case(2, 1)]
    fn when_single_engine_and_apu_running_apu_powers_other_ac_bus(
        #[case] engine: usize,
        #[case] ac_bus: u8,
    ) {
        let test_bed = test_bed_with()
            .running_engine(engine)
            .and()
            .running_apu()
            .run();

        assert!(test_bed
            .ac_bus_output(ac_bus)
            .is_single(PotentialOrigin::ApuGenerator(1)));
    }

    #[test]
    fn when_only_apu_running_apu_powers_ac_bus_1_and_2() {
        let test_bed = test_bed_with().running_apu().run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::ApuGenerator(1)));
    }

    #[rstest]
    #[case(1, 2)]
    #[case(2, 1)]
    fn when_single_engine_running_and_ext_pwr_connected_ext_pwr_powers_other_ac_bus(
        #[case] engine: usize,
        #[case] ac_bus: u8,
    ) {
        let test_bed = test_bed_with()
            .running_engine(engine)
            .connected_external_power()
            .and()
            .ext_pwr_on()
            .run();

        assert!(test_bed
            .ac_bus_output(ac_bus)
            .is_single(PotentialOrigin::External));
    }

    #[test]
    fn when_only_external_power_connected_ext_pwr_powers_ac_bus_1_and_2() {
        let test_bed = test_bed_with()
            .connected_external_power()
            .and()
            .ext_pwr_on()
            .run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::External));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::External));
    }

    #[test]
    fn when_external_power_connected_and_apu_running_external_power_has_priority() {
        let test_bed = test_bed_with()
            .connected_external_power()
            .ext_pwr_on()
            .and()
            .running_apu()
            .run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::External));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::External));
    }

    #[test]
    fn when_both_engines_running_and_external_power_connected_engines_power_ac_buses() {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .connected_external_power()
            .run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(2)));
    }

    #[test]
    fn when_both_engines_running_and_apu_running_engines_power_ac_buses() {
        let test_bed = test_bed_with().running_engines().and().running_apu().run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(2)));
    }

    #[test]
    fn ac_bus_1_powers_ac_ess_bus_whenever_it_is_powered() {
        let test_bed = test_bed_with().running_engines().run();

        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
    }

    #[test]
    fn when_ac_bus_1_becomes_unpowered_but_ac_bus_2_powered_nothing_powers_ac_ess_bus_for_a_while()
    {
        let test_bed = test_bed_with()
            .running_engine(2)
            .and()
            .bus_tie_off()
            .run_waiting_until_just_before_ac_ess_feed_transition();

        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_ess_bus_output().is_unpowered());
    }

    #[test]
    fn when_ac_bus_1_becomes_unpowered_but_ac_bus_2_powered_nothing_powers_dc_ess_bus_for_a_while()
    {
        let test_bed = test_bed_with()
            .running_engine(2)
            .and()
            .bus_tie_off()
            .run_waiting_until_just_before_ac_ess_feed_transition();

        assert!(test_bed.dc_ess_bus_output().is_unpowered());
    }

    #[test]
    fn bat_only_low_airspeed_when_a_single_battery_contactor_closed_static_inverter_has_no_input() {
        let test_bed = test_bed_with()
            .bat_auto(1)
            .bat_off(2)
            .and()
            .airspeed(Velocity::new::<knot>(49.))
            .run_waiting_for(Duration::from_secs(1_000));

        assert!(test_bed.static_inverter_input().is_unpowered());
    }

    #[test]
    fn bat_only_low_airspeed_static_inverter_has_input() {
        let test_bed = test_bed_with()
            .bat_auto(1)
            .bat_auto(2)
            .on_the_ground()
            .and()
            .airspeed(Velocity::new::<knot>(49.))
            .run_waiting_for(Duration::from_secs(1_000));

        assert!(test_bed
            .static_inverter_input()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
    }

    #[test]
    fn when_airspeed_above_50_and_ac_bus_1_and_2_unpowered_and_emergency_gen_off_static_inverter_powers_ac_ess_bus(
    ) {
        let test_bed = test_bed_with()
            .airspeed(Velocity::new::<knot>(51.))
            .run_waiting_for(Duration::from_secs(1_000));

        assert!(test_bed
            .static_inverter_input()
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::StaticInverter));
    }

    /// # Source
    /// Discord (komp#1821):
    /// > The fault light will extinguish after 3 seconds. That's the time delay before automatic switching is activated in case of AC BUS 1 loss.
    #[test]
    fn with_ac_bus_1_being_unpowered_after_a_delay_ac_bus_2_powers_ac_ess_bus() {
        let test_bed = test_bed_with()
            .running_engine(2)
            .and()
            .bus_tie_off()
            .run_waiting_for_ac_ess_feed_transition();

        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
    }

    /// # Source
    /// Discord (komp#1821):
    /// > When AC BUS 1 is available again, it will switch back automatically without delay, unless the AC ESS FEED button is on ALTN.
    #[test]
    fn ac_bus_1_powers_ac_ess_bus_immediately_when_ac_bus_1_becomes_powered_after_ac_bus_2_was_powering_ac_ess_bus(
    ) {
        let test_bed = test_bed_with()
            .running_engine(2)
            .and()
            .bus_tie_off()
            .run_waiting_for_ac_ess_feed_transition()
            .then_continue_with()
            .running_engine(1)
            .and()
            .bus_tie_auto()
            .run();

        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn single_engine_with_generator_off_leaves_ac_buses_unpowered(#[case] engine: usize) {
        let test_bed = test_bed_with()
            .running_engine(engine)
            .and()
            .gen_off(engine)
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
    }

    #[rstest]
    #[case(1, 2)]
    #[case(2, 1)]
    fn when_generator_off_and_both_engines_running_the_other_generator_powers_ac_buses(
        #[case] generator_off: usize,
        #[case] supplying_generator: usize,
    ) {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .gen_off(generator_off)
            .run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(supplying_generator)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(supplying_generator)));
    }

    #[test]
    fn when_ac_ess_feed_push_button_altn_engine_gen_2_powers_ac_ess_bus() {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .ac_ess_feed_altn()
            .run();

        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
    }

    #[test]
    fn when_only_apu_running_but_apu_gen_push_button_off_nothing_powers_ac_bus_1_and_2() {
        let test_bed = test_bed_with().running_apu().and().apu_gen_off().run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
    }

    #[test]
    fn when_only_external_power_connected_but_ext_pwr_push_button_off_nothing_powers_ac_bus_1_and_2(
    ) {
        let test_bed = test_bed_with()
            .connected_external_power()
            .and()
            .ext_pwr_off()
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
    }

    #[test]
    fn when_ac_bus_1_and_ac_bus_2_are_lost_neither_ac_ess_feed_contactor_is_closed() {
        let mut test_bed = test_bed_with().run();

        assert!(test_bed.both_ac_ess_feed_contactors_open());
    }

    #[test]
    fn when_battery_1_full_it_is_not_powered_by_dc_bat_bus() {
        let test_bed = test_bed_with().running_engines().run();

        assert!(test_bed
            .battery_1_input()
            .is_single(PotentialOrigin::Battery(1)));
    }

    #[test]
    fn when_battery_1_not_full_it_is_powered_by_dc_bat_bus() {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .empty_battery_1()
            .run();

        assert!(test_bed.battery_1_input().is_powered());
    }

    #[test]
    fn when_battery_1_not_full_and_button_off_it_is_not_powered_by_dc_bat_bus() {
        let test_bed = test_bed_with()
            .running_engines()
            .empty_battery_1()
            .and()
            .bat_off(1)
            .run();

        assert!(test_bed.battery_1_input().is_unpowered())
    }

    #[test]
    fn when_battery_1_has_charge_powers_hot_bus_1() {
        let test_bed = test_bed().run();

        assert!(test_bed.hot_bus_output(1).is_powered());
    }

    #[test]
    fn when_battery_1_is_empty_and_dc_bat_bus_unpowered_hot_bus_1_unpowered() {
        let test_bed = test_bed_with().empty_battery_1().run();

        assert!(test_bed.hot_bus_output(1).is_unpowered());
    }

    #[test]
    fn when_battery_1_is_empty_and_dc_bat_bus_powered_hot_bus_1_powered() {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .empty_battery_1()
            .run();

        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
    }

    #[test]
    fn when_battery_2_full_it_is_not_powered_by_dc_bat_bus() {
        let test_bed = test_bed_with().running_engines().run();

        assert!(test_bed
            .battery_2_input()
            .is_single(PotentialOrigin::Battery(2)));
    }

    #[test]
    fn when_battery_2_not_full_it_is_powered_by_dc_bat_bus() {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .empty_battery_2()
            .run();

        assert!(test_bed.battery_2_input().is_powered());
    }

    #[test]
    fn when_battery_2_not_full_and_button_off_it_is_not_powered_by_dc_bat_bus() {
        let test_bed = test_bed_with()
            .running_engines()
            .empty_battery_2()
            .and()
            .bat_off(2)
            .run();

        assert!(test_bed.battery_2_input().is_unpowered())
    }

    #[test]
    fn when_battery_2_has_charge_powers_hot_bus_2() {
        let test_bed = test_bed().run();

        assert!(test_bed.hot_bus_output(2).is_powered());
    }

    #[test]
    fn when_battery_2_is_empty_and_dc_bat_bus_unpowered_hot_bus_2_unpowered() {
        let test_bed = test_bed_with().empty_battery_2().run();

        assert!(test_bed.hot_bus_output(2).is_unpowered());
    }

    #[test]
    fn when_battery_2_is_empty_and_dc_bat_bus_powered_hot_bus_2_powered() {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .empty_battery_2()
            .run();

        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
    }

    #[rstest]
    #[case(1, 2)]
    #[case(2, 1)]
    fn when_bus_tie_off_engine_does_not_power_other_ac_bus(
        #[case] engine: usize,
        #[case] ac_bus: u8,
    ) {
        let test_bed = test_bed_with()
            .running_engine(engine)
            .and()
            .bus_tie_off()
            .run();

        assert!(test_bed.ac_bus_output(ac_bus).is_unpowered());
    }

    #[test]
    fn when_bus_tie_off_apu_does_not_power_ac_buses() {
        let test_bed = test_bed_with().running_apu().and().bus_tie_off().run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
    }

    #[test]
    fn when_bus_tie_off_external_power_does_not_power_ac_buses() {
        let test_bed = test_bed_with()
            .connected_external_power()
            .and()
            .bus_tie_off()
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
    }

    #[test]
    fn when_dc_bus_1_and_dc_bus_2_unpowered_dc_bus_2_to_dc_bat_remains_open() {
        let mut test_bed = test_bed().run();

        assert!(test_bed.dc_bus_2_tie_contactor_is_open());
    }

    #[test]
    fn when_ac_ess_bus_powered_ac_ess_feed_does_not_have_fault() {
        let mut test_bed = test_bed_with().running_engines().run();

        assert!(!test_bed.ac_ess_feed_has_fault());
    }

    #[test]
    fn when_ac_ess_bus_is_unpowered_ac_ess_feed_has_fault() {
        let mut test_bed = test_bed_with().airspeed(Velocity::new::<knot>(0.)).run();

        assert!(test_bed.ac_ess_feed_has_fault());
    }

    #[test]
    fn when_single_engine_and_apu_galley_is_not_shed() {
        let mut test_bed = test_bed_with().running_engine(1).and().running_apu().run();

        assert!(!test_bed.galley_is_shed());
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn when_single_engine_gen_galley_is_shed(#[case] engine_number: usize) {
        let mut test_bed = test_bed_with().running_engine(engine_number).run();

        assert!(test_bed.galley_is_shed());
    }

    #[test]
    fn when_on_ground_and_apu_gen_only_galley_is_not_shed() {
        let mut test_bed = test_bed_with().running_apu().and().on_the_ground().run();

        assert!(!test_bed.galley_is_shed());
    }

    #[test]
    fn when_single_engine_gen_with_bus_tie_off_but_apu_running_galley_is_shed() {
        let mut test_bed = test_bed_with()
            .running_engine(1)
            .running_apu()
            .and()
            .bus_tie_off()
            .run();

        assert!(test_bed.galley_is_shed());
    }

    #[test]
    fn when_single_engine_gen_with_bus_tie_off_and_ext_pwr_on_galley_is_shed() {
        let mut test_bed = test_bed_with()
            .running_engine(1)
            .connected_external_power()
            .ext_pwr_on()
            .and()
            .bus_tie_off()
            .run();

        assert!(test_bed.galley_is_shed());
    }

    #[test]
    fn when_on_ground_and_ext_pwr_only_galley_is_not_shed() {
        let mut test_bed = test_bed_with()
            .connected_external_power()
            .ext_pwr_on()
            .and()
            .on_the_ground()
            .run();

        assert!(!test_bed.galley_is_shed());
    }

    #[test]
    fn when_in_flight_and_apu_gen_only_galley_is_shed() {
        let mut test_bed = test_bed_with().running_apu().run();

        assert!(test_bed.galley_is_shed());
    }

    #[test]
    fn when_in_flight_and_emer_gen_only_galley_is_shed() {
        let mut test_bed = test_bed_with().running_emergency_generator().run();

        assert!(test_bed.galley_is_shed());
    }

    #[test]
    fn when_commercial_pb_off_galley_is_shed() {
        let mut test_bed = test_bed_with()
            .running_engines()
            .and()
            .commercial_off()
            .run();

        assert!(test_bed.galley_is_shed());
    }

    #[test]
    fn when_galy_and_cab_pb_off_galley_is_shed() {
        let mut test_bed = test_bed_with()
            .running_engines()
            .and()
            .galy_and_cab_off()
            .run();

        assert!(test_bed.galley_is_shed());
    }

    #[test]
    #[ignore = "Generator overloading is not yet supported."]
    fn when_aircraft_on_the_ground_and_apu_gen_is_overloaded_galley_is_shed() {}

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn when_gen_contactor_open_gen_push_button_has_fault(#[case] gen_number: usize) {
        let mut test_bed = test_bed_with().running_apu().run();

        assert!(test_bed.gen_has_fault(gen_number));
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn when_gen_contactor_open_and_gen_push_button_off_does_not_have_fault(
        #[case] gen_number: usize,
    ) {
        let mut test_bed = test_bed_with()
            .running_apu()
            .and()
            .gen_off(gen_number)
            .run();

        assert!(!test_bed.gen_has_fault(gen_number));
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn when_gen_contactor_closed_gen_push_button_does_not_have_fault(#[case] gen_number: usize) {
        let mut test_bed = test_bed_with().running_engine(gen_number).run();

        assert!(!test_bed.gen_has_fault(gen_number));
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn when_apu_start_with_battery_off_start_contactors_remain_open_and_motor_unpowered(
        #[case] battery_number: usize,
    ) {
        let mut test_bed = test_bed_with()
            .bat_off(battery_number)
            .command_closing_of_start_contactors()
            .and()
            .run_for_start_contactor_test();

        assert!(!test_bed.apu_start_contactors_closed());
        assert!(!test_bed.apu_start_motor_is_powered());
    }

    #[test]
    fn when_apu_start_with_both_batteries_auto_and_closing_commanded_start_contactors_close_and_motor_is_powered(
    ) {
        let mut test_bed = test_bed_with()
            .bat_auto(1)
            .bat_auto(2)
            .command_closing_of_start_contactors()
            .and()
            .run_for_start_contactor_test();

        assert!(test_bed.apu_start_contactors_closed());
        assert!(test_bed.apu_start_motor_is_powered());
    }

    #[test]
    fn when_apu_start_with_both_batteries_auto_and_closing_not_commanded_start_contactors_remain_open_and_motor_unpowered(
    ) {
        let mut test_bed = test_bed_with()
            .bat_auto(1)
            .bat_auto(2)
            .and()
            .run_for_start_contactor_test();

        assert!(!test_bed.apu_start_contactors_closed());
        assert!(!test_bed.apu_start_motor_is_powered());
    }

    #[test]
    fn transitions_between_gen_1_and_gen_2_without_interruption() {
        // The current implementation shouldn't include power interruptions.
        let mut test_bed = test_bed_with()
            .running_engine(1)
            .and()
            .running_engine(2)
            .run();
        assert!(
            test_bed.ac_bus_output(1).is_powered(),
            "Precondition: the test assumes the AC 1 bus is powered at this point."
        );

        test_bed = test_bed.then_continue_with().stopped_engine(1).run_once();

        assert!(test_bed.ac_bus_output(1).is_powered());
    }

    #[test]
    fn when_ac_2_bus_is_powered_it_has_priority_over_ext_pwr_gnd_flt_circuit() {
        let test_bed = test_bed_with()
            .running_engine(2)
            .and()
            .connected_external_power()
            .run();

        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
    }

    #[test]
    fn when_ac_2_bus_is_unpowered_and_ac_1_is_powered_ext_pwr_powers_gnd_flt_buses() {
        let test_bed = test_bed_with()
            .running_engine(1)
            .bus_tie_off()
            .and()
            .connected_external_power()
            .run();

        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::External));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    #[test]
    fn when_gen_1_line_off_and_only_engine_1_running_nothing_powers_ac_buses() {
        let test_bed = test_bed_with()
            .running_engine(1)
            .and()
            .gen_1_line_off()
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
    }

    #[test]
    fn when_gen_1_contactor_open_due_to_gen_1_line_being_off_gen_1_push_button_has_fault() {
        let mut test_bed = test_bed_with()
            .running_engine(1)
            .and()
            .gen_1_line_off()
            .run();

        assert!(test_bed.gen_has_fault(1));
    }

    #[test]
    fn when_emergency_generator_not_supplying_while_ac_1_and_2_unavailable_in_flight_rat_and_emer_gen_has_fault(
    ) {
        let mut test_bed = test_bed_with()
            .running_engines()
            .gen_off(1)
            .and()
            .gen_off(2)
            .run();

        assert!(test_bed.rat_and_emer_gen_has_fault());
    }

    #[test]
    fn when_emergency_generator_not_supplying_while_ac_1_and_2_unavailable_during_takeoff_rat_and_emer_gen_does_not_have_fault(
    ) {
        let mut test_bed = test_bed_with()
            .running_engines()
            .on_the_ground()
            .gen_off(1)
            .and()
            .gen_off(2)
            .run();

        assert!(!test_bed.rat_and_emer_gen_has_fault());
    }

    #[test]
    fn when_emergency_generator_not_supplying_while_ac_1_and_2_unavailable_during_low_speed_flight_rat_and_emer_gen_does_not_have_fault(
    ) {
        let mut test_bed = test_bed_with()
            .running_engines()
            .gen_off(1)
            .gen_off(2)
            .and()
            .airspeed(Velocity::new::<knot>(99.))
            .run();

        assert!(!test_bed.rat_and_emer_gen_has_fault());
    }

    #[test]
    fn when_rat_and_emer_gen_man_on_push_button_is_pressed_at_an_earlier_time_in_case_of_ac_1_and_2_unavailable_emergency_generator_provides_power_immediately(
    ) {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .rat_and_emer_gen_man_on_pressed()
            .run_waiting_for(Duration::from_secs(100))
            .then_continue_with()
            .gen_off(1)
            .and()
            .gen_off(2)
            .run();

        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EmergencyGenerator));
    }

    #[test]
    fn when_rat_and_emer_gen_man_on_push_button_is_pressed_in_case_of_ac_1_and_2_unavailable_emergency_generator_does_not_provide_power_immediately(
    ) {
        let test_bed = test_bed_with()
            .running_engines()
            .gen_off(1)
            .gen_off(2)
            .and()
            .rat_and_emer_gen_man_on_pressed()
            .run_waiting_for(Duration::from_secs(0));

        assert!(!test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EmergencyGenerator));
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn when_engine_fire_push_button_released_and_only_that_engine_is_running_nothing_powers_ac_buses(
        #[case] number: usize,
    ) {
        let test_bed = test_bed_with()
            .running_engine(number)
            .and()
            .released_engine_fire_push_button(number)
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn when_gen_contactor_open_due_to_engine_fire_push_button_released_gen_push_button_has_fault(
        #[case] number: usize,
    ) {
        let mut test_bed = test_bed_with()
            .running_engine(number)
            .and()
            .released_engine_fire_push_button(number)
            .run();

        assert!(test_bed.gen_has_fault(number));
    }

    fn test_bed_with() -> A320ElectricalTestBed {
        test_bed()
    }

    fn test_bed() -> A320ElectricalTestBed {
        A320ElectricalTestBed::new()
    }

    struct TestApu {
        identifier: ElectricalElementIdentifier,
        is_available: bool,
        start_motor_is_powered: bool,
        should_close_start_contactor: bool,
    }
    impl TestApu {
        fn new(identifier_provider: &mut impl ElectricalElementIdentifierProvider) -> Self {
            Self {
                identifier: identifier_provider.next(),
                is_available: false,
                start_motor_is_powered: false,
                should_close_start_contactor: false,
            }
        }

        fn set_available(&mut self, available: bool) {
            self.is_available = available;
        }

        fn command_closing_of_start_contactors(&mut self) {
            self.should_close_start_contactor = true;
        }

        fn start_motor_is_powered(&self) -> bool {
            self.start_motor_is_powered
        }
    }
    impl SimulationElement for TestApu {
        fn receive_power(&mut self, buses: &impl ElectricalBuses) {
            self.start_motor_is_powered = buses.is_powered(ElectricalBusType::Sub("49-42-00"));
        }
    }
    impl AuxiliaryPowerUnitElectrical for TestApu {
        fn output_within_normal_parameters(&self) -> bool {
            self.is_available
        }
    }
    impl ElectricitySource for TestApu {
        fn output_potential(&self) -> Potential {
            if self.is_available {
                Potential::new(
                    PotentialOrigin::ApuGenerator(1),
                    ElectricPotential::new::<volt>(115.),
                )
            } else {
                Potential::none()
            }
        }
    }
    impl ElectricalElement for TestApu {
        fn input_identifier(&self) -> systems::electrical::ElectricalElementIdentifier {
            self.identifier
        }

        fn output_identifier(&self) -> systems::electrical::ElectricalElementIdentifier {
            self.identifier
        }

        fn is_conductive(&self) -> bool {
            true
        }
    }
    impl ApuAvailable for TestApu {
        fn is_available(&self) -> bool {
            self.is_available
        }
    }
    impl ControllerSignal<ContactorSignal> for TestApu {
        fn signal(&self) -> Option<ContactorSignal> {
            if self.should_close_start_contactor {
                Some(ContactorSignal::Close)
            } else {
                Some(ContactorSignal::Open)
            }
        }
    }

    struct TestEngineFirePushButtons {
        is_released: [bool; 2],
    }
    impl TestEngineFirePushButtons {
        fn new() -> Self {
            Self {
                is_released: [false, false],
            }
        }

        fn release(&mut self, engine_number: usize) {
            self.is_released[engine_number - 1] = true;
        }
    }
    impl EngineFirePushButtons for TestEngineFirePushButtons {
        fn is_released(&self, engine_number: usize) -> bool {
            self.is_released[engine_number - 1]
        }
    }

    struct TestEngine {
        is_running: bool,
    }
    impl TestEngine {
        fn new() -> Self {
            Self { is_running: false }
        }

        fn run(&mut self) {
            self.is_running = true;
        }

        fn stop(&mut self) {
            self.is_running = false;
        }
    }
    impl EngineCorrectedN2 for TestEngine {
        fn corrected_n2(&self) -> Ratio {
            Ratio::new::<percent>(if self.is_running { 80. } else { 0. })
        }
    }

    struct TestApuOverhead {
        master_sw_pb_on: bool,
        start_pb_on: bool,
    }
    impl TestApuOverhead {
        fn new() -> Self {
            Self {
                master_sw_pb_on: false,
                start_pb_on: false,
            }
        }

        fn set_apu_master_sw_pb_on(&mut self) {
            self.master_sw_pb_on = true;
        }

        fn set_start_pb_on(&mut self) {
            self.start_pb_on = true;
        }
    }
    impl ApuMaster for TestApuOverhead {
        fn master_sw_is_on(&self) -> bool {
            self.master_sw_pb_on
        }
    }
    impl ApuStart for TestApuOverhead {
        fn start_is_on(&self) -> bool {
            self.start_pb_on
        }
    }

    struct TestHydraulicSystem {
        is_rat_hydraulic_loop_pressurised: bool,
    }
    impl TestHydraulicSystem {
        fn new(is_rat_hydraulic_loop_pressurised: bool) -> Self {
            Self {
                is_rat_hydraulic_loop_pressurised,
            }
        }
    }
    impl RamAirTurbineHydraulicLoopPressurised for TestHydraulicSystem {
        fn is_rat_hydraulic_loop_pressurised(&self) -> bool {
            self.is_rat_hydraulic_loop_pressurised
        }
    }

    struct TestLandingGear {}
    impl TestLandingGear {
        fn new() -> Self {
            Self {}
        }
    }
    impl LandingGearPosition for TestLandingGear {
        fn is_up_and_locked(&self) -> bool {
            true
        }

        fn is_down_and_locked(&self) -> bool {
            false
        }
    }

    struct A320ElectricalTestAircraft {
        engines: [TestEngine; 2],
        ext_pwr: ExternalPowerSource,
        elec: A320Electrical,
        overhead: A320ElectricalOverheadPanel,
        emergency_overhead: A320EmergencyElectricalOverheadPanel,
        apu: TestApu,
        apu_overhead: TestApuOverhead,
        engine_fire_push_buttons: TestEngineFirePushButtons,
    }
    impl A320ElectricalTestAircraft {
        fn new(electricity: &mut Electricity) -> Self {
            Self {
                engines: [TestEngine::new(), TestEngine::new()],
                ext_pwr: ExternalPowerSource::new(electricity),
                elec: A320Electrical::new(electricity),
                overhead: A320ElectricalOverheadPanel::new(),
                emergency_overhead: A320EmergencyElectricalOverheadPanel::new(),
                apu: TestApu::new(electricity),
                apu_overhead: TestApuOverhead::new(),
                engine_fire_push_buttons: TestEngineFirePushButtons::new(),
            }
        }

        fn running_engine(&mut self, number: usize) {
            self.engines[number - 1].run();
        }

        fn stopped_engine(&mut self, number: usize) {
            self.engines[number - 1].stop();
        }

        fn running_apu(&mut self) {
            self.apu.set_available(true);
        }

        fn set_apu_master_sw_pb_on(&mut self) {
            self.apu_overhead.set_apu_master_sw_pb_on();
        }

        fn set_apu_start_pb_on(&mut self) {
            self.apu_overhead.set_start_pb_on();
        }

        fn command_closing_of_start_contactors(&mut self) {
            self.apu.command_closing_of_start_contactors();
        }

        fn apu_start_motor_is_powered(&self) -> bool {
            self.apu.start_motor_is_powered()
        }

        fn empty_battery_1(&mut self) {
            self.elec.empty_battery_1();
        }

        fn empty_battery_2(&mut self) {
            self.elec.empty_battery_2();
        }

        fn failed_tr_1(&mut self) {
            self.elec.fail_tr_1();
        }

        fn failed_tr_2(&mut self) {
            self.elec.fail_tr_2();
        }

        fn running_emergency_generator(&mut self) {
            self.elec.attempt_emergency_gen_start();
        }

        fn static_inverter_input<'a>(&self, electricity: &'a Electricity) -> Ref<'a, Potential> {
            electricity.input_of(self.elec.direct_current.static_inverter())
        }

        fn tr_1_input<'a>(&self, electricity: &'a Electricity) -> Ref<'a, Potential> {
            electricity.input_of(self.elec.tr_1())
        }

        fn tr_2_input<'a>(&self, electricity: &'a Electricity) -> Ref<'a, Potential> {
            electricity.input_of(self.elec.tr_2())
        }

        fn tr_ess_input<'a>(&self, electricity: &'a Electricity) -> Ref<'a, Potential> {
            electricity.input_of(self.elec.tr_ess())
        }

        fn battery_1_input<'a>(&self, electricity: &'a Electricity) -> Ref<'a, Potential> {
            electricity.input_of(self.elec.battery_1())
        }

        fn battery_2_input<'a>(&self, electricity: &'a Electricity) -> Ref<'a, Potential> {
            electricity.input_of(self.elec.battery_2())
        }

        fn release_engine_fire_push_button(&mut self, engine_number: usize) {
            self.engine_fire_push_buttons.release(engine_number);
        }
    }
    impl Aircraft for A320ElectricalTestAircraft {
        fn update_before_power_distribution(
            &mut self,
            context: &UpdateContext,
            electricity: &mut Electricity,
        ) {
            self.elec.update(
                context,
                electricity,
                &self.ext_pwr,
                &self.overhead,
                &self.emergency_overhead,
                &mut self.apu,
                &self.apu_overhead,
                &self.engine_fire_push_buttons,
                [&self.engines[0], &self.engines[1]],
                &TestHydraulicSystem::new(
                    context.indicated_airspeed() > Velocity::new::<knot>(100.),
                ),
                &TestLandingGear::new(),
            );
            self.overhead
                .update_after_electrical(&self.elec, electricity);
            self.emergency_overhead
                .update_after_electrical(context, &self.elec);
        }
    }
    impl SimulationElement for A320ElectricalTestAircraft {
        fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
            self.ext_pwr.accept(visitor);
            self.elec.accept(visitor);
            self.overhead.accept(visitor);
            self.emergency_overhead.accept(visitor);
            self.apu.accept(visitor);

            visitor.visit(self);
        }
    }

    struct A320ElectricalTestBed {
        test_bed: SimulationTestBed<A320ElectricalTestAircraft>,
    }
    impl A320ElectricalTestBed {
        fn new() -> Self {
            Self {
                test_bed: SimulationTestBed::new(|electricity| {
                    A320ElectricalTestAircraft::new(electricity)
                }),
            }
        }

        fn running_engine(mut self, number: usize) -> Self {
            self.command(|a| a.running_engine(number));

            self = self.without_triggering_emergency_elec(|x| {
                x.run_waiting_for(Duration::from_millis(
                    INTEGRATED_DRIVE_GENERATOR_STABILIZATION_TIME_IN_MILLISECONDS,
                ))
            });

            self
        }

        fn stopped_engine(mut self, number: usize) -> Self {
            self.command(|a| a.stopped_engine(number));
            self
        }

        fn running_engines(self) -> Self {
            self.running_engine(1).and().running_engine(2)
        }

        fn running_apu(mut self) -> Self {
            self.command(|a| a.running_apu());
            self
        }

        fn connected_external_power(mut self) -> Self {
            self.write("EXTERNAL POWER AVAILABLE:1", true);

            self.without_triggering_emergency_elec(|x| x.run())
        }

        fn empty_battery_1(mut self) -> Self {
            self.command(|a| a.empty_battery_1());
            self
        }

        fn empty_battery_2(mut self) -> Self {
            self.command(|a| a.empty_battery_2());
            self
        }

        fn airspeed(mut self, ias: Velocity) -> Self {
            self.set_indicated_airspeed(ias);
            self
        }

        fn on_the_ground(mut self) -> Self {
            self.set_indicated_altitude(Length::new::<foot>(0.));
            self.set_on_ground(true);
            self
        }

        fn run_for_start_contactor_test(self) -> Self {
            self.airspeed(Velocity::new::<knot>(0.))
                .on_the_ground()
                .apu_master_sw_pb_on()
                .and()
                .apu_start_pb_on()
                .run()
        }

        fn and(self) -> Self {
            self
        }

        fn then_continue_with(self) -> Self {
            self
        }

        fn failed_tr_1(mut self) -> Self {
            self.command(|a| a.failed_tr_1());
            self
        }

        fn failed_tr_2(mut self) -> Self {
            self.command(|a| a.failed_tr_2());
            self
        }

        fn running_emergency_generator(mut self) -> Self {
            self.command(|a| a.running_emergency_generator());
            self.run_waiting_for(Duration::from_secs(100))
        }

        fn gen_off(mut self, number: usize) -> Self {
            self.write(&format!("OVHD_ELEC_ENG_GEN_{}_PB_IS_ON", number), false);
            self
        }

        fn released_engine_fire_push_button(mut self, engine_number: usize) -> Self {
            self.command(|a| a.release_engine_fire_push_button(engine_number));
            self
        }

        fn gen_1_line_off(mut self) -> Self {
            self.write("OVHD_EMER_ELEC_GEN_1_LINE_PB_IS_ON", false);
            self
        }

        fn apu_gen_off(mut self) -> Self {
            self.write("OVHD_ELEC_APU_GEN_PB_IS_ON", false);
            self
        }

        fn ext_pwr_on(mut self) -> Self {
            self.write("OVHD_ELEC_EXT_PWR_PB_IS_ON", true);
            self
        }

        fn ext_pwr_off(mut self) -> Self {
            self.write("OVHD_ELEC_EXT_PWR_PB_IS_ON", false);
            self
        }

        fn ac_ess_feed_altn(mut self) -> Self {
            self.write("OVHD_ELEC_AC_ESS_FEED_PB_IS_NORMAL", false);
            self
        }

        fn bat_off(self, number: usize) -> Self {
            self.bat(number, false)
        }

        fn bat_auto(self, number: usize) -> Self {
            self.bat(number, true)
        }

        fn bat(mut self, number: usize, auto: bool) -> Self {
            self.write(&format!("OVHD_ELEC_BAT_{}_PB_IS_AUTO", number), auto);
            self
        }

        fn bus_tie(mut self, auto: bool) -> Self {
            self.write("OVHD_ELEC_BUS_TIE_PB_IS_AUTO", auto);
            self
        }

        fn bus_tie_auto(self) -> Self {
            self.bus_tie(true)
        }

        fn bus_tie_off(self) -> Self {
            self.bus_tie(false)
        }

        fn commercial_off(mut self) -> Self {
            self.write("OVHD_ELEC_COMMERCIAL_PB_IS_ON", false);
            self
        }

        fn galy_and_cab_off(mut self) -> Self {
            self.write("OVHD_ELEC_GALY_AND_CAB_PB_IS_AUTO", false);
            self
        }

        fn apu_master_sw_pb_on(mut self) -> Self {
            self.command(|a| a.set_apu_master_sw_pb_on());
            self
        }

        fn apu_start_pb_on(mut self) -> Self {
            self.command(|a| a.set_apu_start_pb_on());
            self
        }

        fn rat_and_emer_gen_man_on_pressed(mut self) -> Self {
            self.write("OVHD_EMER_ELEC_RAT_AND_EMER_GEN_IS_PRESSED", true);
            self
        }

        fn command_closing_of_start_contactors(mut self) -> Self {
            self.command(|a| a.command_closing_of_start_contactors());
            self
        }

        fn apu_start_contactors_closed(&mut self) -> bool {
            self.read("ELEC_CONTACTOR_10KA_AND_5KA_IS_CLOSED")
        }

        fn apu_start_motor_is_powered(&self) -> bool {
            self.query(|a| a.apu_start_motor_is_powered())
        }

        fn ac_bus_output(&self, number: u8) -> Ref<Potential> {
            self.query_elec_ref(|_, elec| {
                elec.potential_of(ElectricalBusType::AlternatingCurrent(number))
            })
        }

        fn ac_ess_bus_output(&self) -> Ref<Potential> {
            self.query_elec_ref(|_, elec| {
                elec.potential_of(ElectricalBusType::AlternatingCurrentEssential)
            })
        }

        fn ac_ess_shed_bus_output(&self) -> Ref<Potential> {
            self.query_elec_ref(|_, elec| {
                elec.potential_of(ElectricalBusType::AlternatingCurrentEssentialShed)
            })
        }

        fn ac_stat_inv_bus_output(&self) -> Ref<Potential> {
            self.query_elec_ref(|_, elec| {
                elec.potential_of(ElectricalBusType::AlternatingCurrentStaticInverter)
            })
        }

        fn ac_gnd_flt_service_bus_output(&self) -> Ref<Potential> {
            self.query_elec_ref(|_, elec| {
                elec.potential_of(ElectricalBusType::AlternatingCurrentGndFltService)
            })
        }

        fn static_inverter_input(&self) -> Ref<Potential> {
            self.query_elec_ref(|a, elec| a.static_inverter_input(elec))
        }

        fn tr_1_input(&self) -> Ref<Potential> {
            self.query_elec_ref(|a, elec| a.tr_1_input(elec))
        }

        fn tr_2_input(&self) -> Ref<Potential> {
            self.query_elec_ref(|a, elec| a.tr_2_input(elec))
        }

        fn tr_ess_input(&self) -> Ref<Potential> {
            self.query_elec_ref(|a, elec| a.tr_ess_input(elec))
        }

        fn battery_1_input(&self) -> Ref<Potential> {
            self.query_elec_ref(|a, elec| a.battery_1_input(elec))
        }

        fn battery_2_input(&self) -> Ref<Potential> {
            self.query_elec_ref(|a, elec| a.battery_2_input(elec))
        }

        fn dc_bus_output(&self, number: u8) -> Ref<Potential> {
            self.query_elec_ref(|_, elec| {
                elec.potential_of(ElectricalBusType::DirectCurrent(number))
            })
        }

        fn dc_bat_bus_output(&self) -> Ref<Potential> {
            self.query_elec_ref(|_, elec| {
                elec.potential_of(ElectricalBusType::DirectCurrentBattery)
            })
        }

        fn dc_ess_bus_output(&self) -> Ref<Potential> {
            self.query_elec_ref(|_, elec| {
                elec.potential_of(ElectricalBusType::DirectCurrentEssential)
            })
        }

        fn dc_ess_shed_bus_output(&self) -> Ref<Potential> {
            self.query_elec_ref(|_, elec| {
                elec.potential_of(ElectricalBusType::DirectCurrentEssentialShed)
            })
        }

        fn hot_bus_output(&self, number: u8) -> Ref<Potential> {
            self.query_elec_ref(|_, elec| {
                elec.potential_of(ElectricalBusType::DirectCurrentHot(number))
            })
        }

        fn dc_gnd_flt_service_bus_output(&self) -> Ref<Potential> {
            self.query_elec_ref(|_, elec| {
                elec.potential_of(ElectricalBusType::DirectCurrentGndFltService)
            })
        }

        fn ac_ess_feed_has_fault(&mut self) -> bool {
            self.read("OVHD_ELEC_AC_ESS_FEED_PB_HAS_FAULT")
        }

        fn gen_has_fault(&mut self, number: usize) -> bool {
            self.read(&format!("OVHD_ELEC_ENG_GEN_{}_PB_HAS_FAULT", number))
        }

        fn rat_and_emer_gen_has_fault(&mut self) -> bool {
            self.read("OVHD_EMER_ELEC_RAT_AND_EMER_GEN_HAS_FAULT")
        }

        fn galley_is_shed(&mut self) -> bool {
            self.read("ELEC_GALLEY_IS_SHED")
        }

        fn both_ac_ess_feed_contactors_open(&mut self) -> bool {
            !Read::<bool>::read(self, "ELEC_CONTACTOR_3XC1_IS_CLOSED")
                && !Read::<bool>::read(self, "ELEC_CONTACTOR_3XC2_IS_CLOSED")
        }

        fn dc_bus_2_tie_contactor_is_open(&mut self) -> bool {
            !Read::<bool>::read(self, "ELEC_CONTACTOR_1PC2_IS_CLOSED")
        }

        fn run(self) -> Self {
            self.run_waiting_for(Duration::from_secs(1))
        }

        fn run_waiting_for(mut self, delta: Duration) -> Self {
            self.run_with_delta(delta);

            // Sadly it's impossible for some electrical origins such as
            // the generators to know their output potential before a single
            // simulation tick has passed, as the output potential among other
            // things depends on electrical load which is only known near the
            // end of a tick. As the electrical system disallows e.g. an engine
            // generator contactor to close when its electrical parameters are
            // outside of normal parameters, we have to run a second tick before
            // the potential has flown through the system in the way we expected.
            self.run_with_delta(Duration::from_secs(0));

            self
        }

        /// Runs the simulation a single time with a delta of 1 second.
        /// This particular is useful for tests that want to verify behaviour
        /// which only occurs in a single tick and would be hidden by
        /// run or run_waiting_for, which executes two ticks.
        fn run_once(mut self) -> Self {
            self.run_with_delta(Duration::from_secs(1));

            self
        }

        /// Tests assume they start at altitude with high velocity.
        /// As power sources can take some time before they become available
        /// by wrapping the functions that enable such a power sources we ensure it cannot
        /// trigger the deployment of the RAT or start of EMER GEN.
        fn without_triggering_emergency_elec(mut self, mut func: impl FnMut(Self) -> Self) -> Self {
            let ias = self.indicated_airspeed();
            self.set_indicated_airspeed(Velocity::new::<knot>(0.));

            self = func(self);

            self.set_indicated_airspeed(ias);

            self
        }

        fn run_waiting_for_ac_ess_feed_transition(self) -> Self {
            self.run_waiting_for(A320AcEssFeedContactors::AC_ESS_FEED_TO_AC_BUS_2_DELAY_IN_SECONDS)
        }

        fn run_waiting_until_just_before_ac_ess_feed_transition(self) -> Self {
            self.run_waiting_for(
                A320AcEssFeedContactors::AC_ESS_FEED_TO_AC_BUS_2_DELAY_IN_SECONDS
                    - Duration::from_millis(1),
            )
        }
    }
    impl TestBed for A320ElectricalTestBed {
        type Aircraft = A320ElectricalTestAircraft;

        fn test_bed(&self) -> &SimulationTestBed<A320ElectricalTestAircraft> {
            &self.test_bed
        }

        fn test_bed_mut(&mut self) -> &mut SimulationTestBed<A320ElectricalTestAircraft> {
            &mut self.test_bed
        }
    }
}
