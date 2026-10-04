# Makefile — point d'entrée unique du projet light-flash.
# Convention : toute commande d'installation, compilation, test, flash ou moniteur
# passe par ici (voir CLAUDE.md). `make help` liste les cibles.
#
# Tant qu'il n'y a pas de workspace Cargo, chaque crate se compile depuis son
# propre répertoire : la variable CRATE sélectionne la crate (défaut : light-flash).
#
#   make setup                              installe la chaîne d'outils (Ubuntu)
#   make build  CRATE=hardware-check        compile en debug
#   make run    RELEASE=1 PORT=/dev/ttyACM0 flash + moniteur série
#   make example CRATE=rgb-led EX=ws2812    lance un exemple d'une lib

SHELL := /bin/bash
.DEFAULT_GOAL := help

# ---- Paramètres ------------------------------------------------------------
CRATE   ?= light-flash
PORT    ?=
RELEASE ?= 0
EX      ?=
CHIP    := esp32c3
TARGET  := riscv32imc-esp-espidf
DIST    := dist

CRATE_DIRS := light-flash hardware-check common/lib/rgb-led common/lib/wifi
CRATE_DIR  := $(filter %/$(CRATE) $(CRATE),$(CRATE_DIRS))
ifeq ($(CRATE_DIR),)
$(error CRATE inconnue : "$(CRATE)". Valeurs possibles : $(notdir $(CRATE_DIRS)))
endif

# Toolchain lue dans rust-toolchain.toml pour ne pas la dupliquer ici.
TOOLCHAIN := $(shell sed -n 's/^channel *= *"\(.*\)"/\1/p' light-flash/rust-toolchain.toml)

ifeq ($(RELEASE),1)
PROFILE     := release
CARGO_FLAGS += --release
else
PROFILE     := debug
endif

ifneq ($(PORT),)
PORT_FLAG := --port $(PORT)
endif

ELF   := $(CRATE_DIR)/target/$(TARGET)/$(PROFILE)/$(CRATE)
CARGO := cd $(CRATE_DIR) && cargo

.PHONY: help setup setup-system setup-rust setup-serial doctor \
        build release check clippy fmt fmt-check lint test clean clean-all \
        run flash monitor erase example image size

# ---- Aide ------------------------------------------------------------------
help: ## Affiche cette aide
	@grep -hE '^[a-zA-Z0-9_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'
	@echo
	@echo "Variables : CRATE=$(CRATE) ($(notdir $(CRATE_DIRS)))  RELEASE=$(RELEASE)  PORT=$(PORT)  EX=$(EX)"

# ---- Installation (Ubuntu) -------------------------------------------------
setup: setup-system setup-rust setup-serial doctor ## Installe tout : paquets, toolchain Rust, outils ESP, accès série

setup-system: ## Paquets Ubuntu requis par esp-idf-sys, bindgen et espflash
	sudo apt-get update
	sudo apt-get install -y build-essential curl git python3 python3-pip python3-venv \
	    cmake ninja-build clang libclang-dev pkg-config libssl-dev libudev-dev

setup-rust: ## Toolchain nightly épinglée + rust-src + ldproxy + espflash (pas besoin d'espup sur RISC-V)
	rustup toolchain install $(TOOLCHAIN) --component rust-src
	cargo install --locked ldproxy espflash cargo-espflash

setup-serial: ## Accès à /dev/ttyACM0 : groupe dialout + règle udev (se reconnecter ensuite)
	sudo usermod -aG dialout $$USER
	printf 'SUBSYSTEM=="tty", ATTRS{idVendor}=="303a", ATTRS{idProduct}=="1001", MODE="0660", GROUP="dialout"\n' \
	    | sudo tee /etc/udev/rules.d/99-espressif.rules >/dev/null
	sudo udevadm control --reload-rules && sudo udevadm trigger
	@echo ">> Se déconnecter puis se reconnecter pour que le groupe dialout soit pris en compte."

doctor: ## Vérifie la présence des outils et de la carte
	@echo "Toolchain attendue : $(TOOLCHAIN)"
	@rustup toolchain list 2>/dev/null | grep -q "$(TOOLCHAIN)" && echo "  [ok] toolchain" || echo "  [!!] toolchain absente : make setup-rust"
	@for t in ldproxy espflash cargo-espflash; do command -v $$t >/dev/null && echo "  [ok] $$t" || echo "  [!!] $$t absent : make setup-rust"; done
	@for p in cmake ninja clang python3 pkg-config; do command -v $$p >/dev/null && echo "  [ok] $$p" || echo "  [!!] $$p absent : make setup-system"; done
	@id -nG | grep -qw dialout && echo "  [ok] groupe dialout" || echo "  [!!] pas dans le groupe dialout : make setup-serial"
	@if ls /dev/ttyACM* >/dev/null 2>&1; then ls /dev/ttyACM* | sed 's/^/  [ok] carte : /'; else echo "  [--] aucune carte détectée (/dev/ttyACM*)"; fi

# ---- Compilation -----------------------------------------------------------
build: ## Compile la crate (CRATE=..., RELEASE=1 pour le profil release)
	$(CARGO) build $(CARGO_FLAGS)

release: ## Compile en release
	$(MAKE) build RELEASE=1 CRATE=$(CRATE)

check: ## cargo check (plus rapide que build)
	$(CARGO) check $(CARGO_FLAGS)

clippy: ## Lint (warnings = erreurs)
	$(CARGO) clippy $(CARGO_FLAGS) -- -D warnings

fmt: ## Formate le code
	$(CARGO) fmt

fmt-check: ## Vérifie le formatage sans modifier
	$(CARGO) fmt -- --check

lint: fmt-check clippy ## fmt-check + clippy

test: ## Tests sur l'hôte (aucune crate testable pour l'instant, voir BACKLOG.md §2.2)
	@echo "Aucun test hôte pour l'instant : les crates sans dépendance ESP (light-core, improv) seront branchées ici."

clean: ## Nettoie le target de la crate
	$(CARGO) clean

clean-all: ## Nettoie toutes les crates et dist/
	for d in $(CRATE_DIRS); do (cd $$d && cargo clean); done
	rm -rf $(DIST)

# ---- Flash / moniteur ------------------------------------------------------
run: ## Compile, flashe et ouvre le moniteur série (= cargo run, runner espflash)
	$(CARGO) run $(CARGO_FLAGS) -- $(PORT_FLAG)

flash: build ## Flashe l'ELF sans ouvrir le moniteur
	espflash flash --chip $(CHIP) $(PORT_FLAG) $(ELF)

monitor: ## Moniteur série seul
	espflash monitor $(PORT_FLAG)

erase: ## Efface toute la flash (après changement de table de partitions)
	espflash erase-flash $(PORT_FLAG)

example: ## Lance un exemple d'une lib : make example CRATE=rgb-led EX=ws2812
	@test -n "$(EX)" || { echo "EX=<nom de l'exemple> requis (ex. make example CRATE=rgb-led EX=ws2812)"; exit 1; }
	$(CARGO) run $(CARGO_FLAGS) --example $(EX) -- $(PORT_FLAG)

image: ## Image flashable autonome (bootloader + partitions + app) dans dist/
	mkdir -p $(DIST)
	cd $(CRATE_DIR) && cargo espflash save-image --chip $(CHIP) --merge $(CARGO_FLAGS) $(CURDIR)/$(DIST)/$(CRATE)-$(PROFILE).bin
	@ls -l $(DIST)/$(CRATE)-$(PROFILE).bin

size: build ## Taille de l'ELF produit
	@ls -l $(ELF)
