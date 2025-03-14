class EICASCommonDisplay extends Airliners.EICASTemplateElement {
    constructor() {
        super();
        this.isInitialised = false;
    }
    get templateID() {
        return "EICASCommonDisplayTemplate";
    }
    connectedCallback() {
        super.connectedCallback();
        TemplateElement.call(this, this.init.bind(this));
    }
    init() {
        this.tatText = this.querySelector("#TATValue");
        this.satText = this.querySelector("#SATValue");
        this.isaText = this.querySelector("#ISAValue");
        this.isaContainer = this.querySelector("#ISA");
        this.areAdirsAligned = null;
        this.isSATVisible = null;
        this.isTATVisible = null;
        this.isISAVisible = null;
        this.currentSeconds = 0;
        this.currentMinutes = 0;
        this.hoursText = this.querySelector("#HoursValue");
        this.minutesText = this.querySelector("#MinutesValue");
        this.loadFactorContainer = this.querySelector("#LoadFactor");
        this.loadFactorText = this.querySelector("#LoadFactorValue");
        this.loadFactorSet = new NXLogic_ConfirmNode(2);
        this.loadFactorReset = new NXLogic_ConfirmNode(5);
        this.loadFactorVisible = new NXLogic_MemoryNode(true);
        this.gwUnit = this.querySelector("#GWUnit");
        this.gwValue = this.querySelector("#GWValue");
        this.conversionWeight = parseFloat(NXDataStore.get("CONFIG_USING_METRIC_UNIT", "1"));
        this.gwUnit.textContent = this.conversionWeight === 1 ? "KG" : "LBS";
        this.refreshTAT(0);
        this.refreshSAT(0);
        this.refreshISA(0);
        this.refreshClock();
        this.refreshGrossWeight(true);
        this.isInitialised = true;
    }
    update(_deltaTime) {
        if (!this.isInitialised) {
            return;
        }
        this.refreshTAT(Math.round(Simplane.getTotalAirTemperature()));
        this.refreshSAT(Math.round(Simplane.getAmbientTemperature()));
        this.refreshISA(Math.round(A32NX_Util.getIsaTempDeviation()));
        this.refreshClock();
        this.refreshLoadFactor(_deltaTime, SimVar.GetSimVarValue("G FORCE", "GFORCE"));
        this.refreshGrossWeight();
        this.refreshADIRS();
    }
    refreshTAT(_value) {
        if (_value === this.currentTAT && this.areAdirsAligned === this.isTATVisible) {
            return;
        }

        this.currentTAT = _value;
        this.isTATVisible = this.areAdirsAligned;

        if (this.isTATVisible && this.tatText != null) {
            if (this.currentTAT > 0) {
                this.tatText.textContent = "+" + this.currentTAT.toString();
            } else {
                this.tatText.textContent = this.currentTAT.toString();
            }
        }
    }
    refreshSAT(_value) {
        if (_value === this.currentSAT && this.areAdirsAligned === this.isSATVisible) {
            return;
        }

        this.currentSAT = _value;
        this.isSATVisible = this.areAdirsAligned;

        if (this.isSATVisible && this.satText != null) {
            if (this.currentSAT > 0) {
                this.satText.textContent = "+" + this.currentSAT.toString();
            } else {
                this.satText.textContent = this.currentSAT.toString();
            }
        }
    }
    refreshISA(_value) {
        const isInStdMode = Simplane.getPressureSelectedMode(Aircraft.A320_NEO) === "STD";
        const shouldISABeVisible = isInStdMode && this.areAdirsAligned;

        // Only perform DOM update if visibility changed
        if (this.isaContainer && this.isISAVisible !== shouldISABeVisible) {
            this.isaContainer.setAttribute("visibility", shouldISABeVisible ? "visible" : "hidden");
            this.isISAVisible = shouldISABeVisible;
        }

        if (_value === this.currentISA) {
            return;
        }

        this.currentISA = _value;
        if (this.isaText != null) {
            if (this.currentISA > 0) {
                this.isaText.textContent = "+" + this.currentISA.toString();
            } else {
                this.isaText.textContent = this.currentISA.toString();
            }
        }
    }
    refreshLoadFactor(_deltaTime, value) {
        const conditionsMet = value > 1.4 || value < 0.7;
        const loadFactorSet = this.loadFactorSet.write(conditionsMet, _deltaTime);
        const loadFactorReset = this.loadFactorReset.write(!conditionsMet, _deltaTime);
        const flightPhase = SimVar.GetSimVarValue("L:A32NX_FWC_FLIGHT_PHASE", "Enum");
        const isVisible = (
            flightPhase >= 4 &&
            flightPhase <= 8 &&
            this.loadFactorVisible.write(loadFactorSet, loadFactorReset)
        );

        if (this.loadFactorContainer) {
            if (!isVisible) {
                this.loadFactorContainer.setAttribute("visibility", "hidden");
                if (this.loadFactorText) {
                    this.loadFactorText.textContent = "";
                }
                return;
            }
            this.loadFactorContainer.setAttribute("visibility", "visible");
        }

        if (this.loadFactorText) {
            const clamped = Math.min(Math.max(value, -3), 5);
            this.loadFactorText.textContent = (clamped >= 0 ? "+" : "") + clamped.toFixed(1);
        }
    }
    refreshClock() {
        const seconds = Math.floor(SimVar.GetGlobalVarValue("ZULU TIME", "seconds"));
        if (seconds != this.currentSeconds) {
            this.currentSeconds = seconds;
            const hours = Math.floor(seconds / 3600);
            const minutes = Math.floor((seconds - (hours * 3600)) / 60);
            if (minutes != this.currentMinutes) {
                this.currentMinutes = minutes;
                if (this.hoursText != null) {
                    this.hoursText.textContent = hours.toString().padStart(2, "0");
                }
                if (this.minutesText != null) {
                    this.minutesText.textContent = minutes.toString().padStart(2, "0");
                }
            }
        }
    }
    refreshGrossWeight(_force = false) {
        const fuelWeight = SimVar.GetSimVarValue("FUEL TOTAL QUANTITY WEIGHT", "kg");
        const emptyWeight = SimVar.GetSimVarValue("EMPTY WEIGHT", "kg");
        const payloadWeight = this.getPayloadWeight("kg");
        const gw = Math.round((emptyWeight + fuelWeight + payloadWeight) * this.conversionWeight);
        if ((gw != this.currentGW) || _force) {
            this.currentGW = gw;
            if (this.gwValue != null) {
                // Lower EICAS displays GW in increments of 100
                this.gwValue.textContent = (Math.floor(this.currentGW / 100) * 100).toString();
            }
        }
    }
    getPayloadWeight(unit) {
        const payloadCount = SimVar.GetSimVarValue("PAYLOAD STATION COUNT", "number");
        let payloadWeight = 0;
        for (let i = 1; i <= payloadCount; i++) {
            payloadWeight += SimVar.GetSimVarValue(`PAYLOAD STATION WEIGHT:${i}`, unit);
        }
        return payloadWeight;
    }
    refreshADIRS() {
        if (this.tatText != null && this.satText != null) {
            const areAdirsAlignedNow = SimVar.GetSimVarValue("L:A320_Neo_ADIRS_STATE", "Enum") == 2;

            if (areAdirsAlignedNow === this.areAdirsAligned) {
                return;
            }

            this.areAdirsAligned = areAdirsAlignedNow;

            if (!this.areAdirsAligned) {
                this.tatText.textContent = "XX";
                this.tatText.classList.add("Warning");
                this.tatText.classList.remove("Value");
                this.satText.textContent = "XX";
                this.satText.classList.add("Warning");
                this.satText.classList.remove("Value");
            } else {
                this.satText.classList.add("Value");
                this.satText.classList.remove("Warning");
                this.tatText.classList.add("Value");
                this.tatText.classList.remove("Warning");
                this.isaText.classList.add("Value");
                this.isaText.classList.remove("Warning");
            }
        }
    }
}
customElements.define("eicas-common-display", EICASCommonDisplay);
