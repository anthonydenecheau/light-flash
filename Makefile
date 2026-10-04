# Makefile — point d'entrée unique du projet light-flash.
# Convention : toute commande d'installation, compilation, test, flash ou moniteur
# passe par ici (voir CLAUDE.md). `make help` liste les cibles.
#
#   make setup                              installe la chaîne d'outils (Ubuntu)
#   make build  CRATE=hardware-check        compile une crate du workspace
#   make run    RELEASE=1 PORT=/dev/ttyACM0 flash + moniteur série
#   make example CRATE=rgb-led EX=ws2812    lance un exemple d'une lib
#   make test                               tests sur l'hôte (crates sans dépendance ESP)

SHELL := /bin/bash
.DEFAULT_GOAL := help
# rustup/cargo ne sont pas forcément dans le PATH d'un shell non interactif (CI, cron, agents).
export PATH := $(HOME)/.cargo/bin:$(PATH)

# ---- Paramètres ------------------------------------------------------------
CRATE   ?= light-flash
PORT    ?=
RELEASE ?= 0
EX      ?=
CHIP    := esp32c3
TARGET  := riscv32imc-esp-espidf
DIST    := dist

# Crates du workspace, et sous-ensemble testable sur l'hôte (sans dépendance ESP).
BINS        := light-flash hardware-check
LIBS        := light-core rgb-led wifi
HOST_CRATES := light-core
CRATES      := $(BINS) $(LIBS)
ifeq ($(filter $(CRATE),$(CRATES)),)
$(error CRATE inconnue : "$(CRATE)". Valeurs possibles : $(CRATES))
endif

# Toolchain lue dans rust-toolchain.toml pour ne pas la dupliquer ici.
TOOLCHAIN := $(shell sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)
# Les tests hôte passent par la toolchain stable : elle ignore la section
# [unstable] build-std du .cargo/config.toml, qui ne concerne que la cible ESP.
HOST_TARGET := $(shell rustc +stable -vV 2>/dev/null | sed -n 's/^host: //p')

ifeq ($(RELEASE),1)
PROFILE     := release
CARGO_FLAGS += --release
else
PROFILE     := debug
endif

ifneq ($(PORT),)
PORT_FLAG := --port $(PORT)
endif

ELF := target/$(TARGET)/$(PROFILE)/$(CRATE)

# hardware-check exige cfg.toml (identifiants Wi-Fi) à la racine : sans ce fichier, son build.rs
# fait échouer tout le workspace, on l'exclut donc des cibles globales.
WS_EXCLUDE := $(if $(wildcard cfg.toml),,--exclude hardware-check)

.PHONY: help setup setup-system setup-rust setup-serial doctor \
        build build-all release check clippy fmt fmt-check lint test clean \
        run flash monitor erase example image size

# ---- Aide ------------------------------------------------------------------
help: ## Affiche cette aide
	@grep -hE '^[a-zA-Z0-9_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'
	@echo
	@echo "Variables : CRATE=$(CRATE) ($(CRATES))  RELEASE=$(RELEASE)  PORT=$(PORT)  EX=$(EX)"

# ---- Installation (Ubuntu) -------------------------------------------------
setup: setup-system setup-rust setup-serial doctor ## Installe tout : paquets, toolchain Rust, outils ESP, accès série

setup-system: ## Paquets Ubuntu requis par esp-idf-sys, bindgen et espflash (sudo)
	sudo apt-get update
	sudo apt-get install -y build-essential curl git python3 python3-pip python3-venv \
	    cmake ninja-build clang libclang-dev pkg-config libssl-dev libudev-dev

setup-rust: ## Toolchains stable (tests hôte) et nightly épinglée (cible) + ldproxy + espflash
	rustup toolchain install stable
	rustup toolchain install $(TOOLCHAIN) --profile minimal --component rust-src,rustfmt,clippy
	# Avec la toolchain stable : espflash exige un rustc plus récent que le nightly épinglé.
	cargo +stable install --locked ldproxy espflash cargo-espflash

setup-serial: ## Accès à /dev/ttyACM0 : groupe dialout + règle udev (sudo, se reconnecter ensuite)
	sudo usermod -aG dialout $$USER
	printf 'SUBSYSTEM=="tty", ATTRS{idVendor}=="303a", ATTRS{idProduct}=="1001", MODE="0660", GROUP="dialout"\n' \
	    | sudo tee /etc/udev/rules.d/99-espressif.rules >/dev/null
	sudo udevadm control --reload-rules && sudo udevadm trigger
	@echo ">> Se déconnecter puis se reconnecter pour que le groupe dialout soit pris en compte."

doctor: ## Vérifie la présence des outils et de la carte
	@echo "Toolchain cible attendue : $(TOOLCHAIN)"
	@rustup toolchain list 2>/dev/null | grep -q "$(TOOLCHAIN)" && echo "  [ok] toolchain $(TOOLCHAIN)" || echo "  [!!] toolchain $(TOOLCHAIN) absente : make setup-rust"
	@rustup toolchain list 2>/dev/null | grep -q "^stable" && echo "  [ok] toolchain stable (tests hôte)" || echo "  [!!] toolchain stable absente : make setup-rust"
	@for t in ldproxy espflash cargo-espflash; do command -v $$t >/dev/null && echo "  [ok] $$t" || echo "  [!!] $$t absent : make setup-rust"; done
	@for p in cmake ninja clang python3 pkg-config; do command -v $$p >/dev/null && echo "  [ok] $$p" || echo "  [!!] $$p absent : make setup-system"; done
	@id -nG | grep -qw dialout && echo "  [ok] groupe dialout" || echo "  [!!] pas dans le groupe dialout : make setup-serial"
	@if ls /dev/ttyACM* >/dev/null 2>&1; then \
	    for d in /dev/ttyACM*; do if [ -w $$d ]; then echo "  [ok] carte : $$d"; else echo "  [!!] carte $$d non accessible en écriture (groupe dialout : se reconnecter)"; fi; done; \
	else echo "  [--] aucune carte détectée (/dev/ttyACM*)"; fi

# ---- Compilation -----------------------------------------------------------
build: ## Compile la crate (CRATE=..., RELEASE=1 pour le profil release)
	cargo build -p $(CRATE) $(CARGO_FLAGS)

build-all: ## Compile tout le workspace, exemples compris (hardware-check seulement si cfg.toml existe)
	@test -f cfg.toml || echo ">> cfg.toml absent : hardware-check exclu (cp cfg.toml.example cfg.toml puis renseigner)"
	cargo build --workspace $(WS_EXCLUDE) --bins --examples $(CARGO_FLAGS)

release: ## Compile en release
	$(MAKE) build RELEASE=1 CRATE=$(CRATE)

check: ## cargo check sur tout le workspace (plus rapide que build)
	cargo check --workspace $(WS_EXCLUDE) --bins --examples $(CARGO_FLAGS)

clippy: ## Lint de tout le workspace (warnings = erreurs)
	cargo clippy --workspace $(WS_EXCLUDE) --bins --examples $(CARGO_FLAGS) -- -D warnings

fmt: ## Formate le code
	cargo fmt --all

fmt-check: ## Vérifie le formatage sans modifier
	cargo fmt --all -- --check

lint: fmt-check clippy ## fmt-check + clippy

test: ## Tests sur l'hôte des crates sans dépendance ESP ($(HOST_CRATES))
	@test -n "$(HOST_TARGET)" || { echo "toolchain stable absente : make setup-rust"; exit 1; }
	for c in $(HOST_CRATES); do cargo +stable test -p $$c --target $(HOST_TARGET) || exit 1; done

clean: ## Nettoie target/ et dist/
	cargo clean
	rm -rf $(DIST)

# ---- Flash / moniteur ------------------------------------------------------
run: ## Compile, flashe et ouvre le moniteur série (= cargo run, runner espflash)
	cargo run -p $(CRATE) $(CARGO_FLAGS) -- $(PORT_FLAG)

flash: build ## Flashe l'ELF sans ouvrir le moniteur
	espflash flash --chip $(CHIP) $(PORT_FLAG) $(ELF)

monitor: ## Moniteur série seul
	espflash monitor $(PORT_FLAG)

erase: ## Efface toute la flash (après changement de table de partitions)
	espflash erase-flash $(PORT_FLAG)

example: ## Lance un exemple d'une lib : make example CRATE=rgb-led EX=ws2812
	@test -n "$(EX)" || { echo "EX=<nom de l'exemple> requis (ex. make example CRATE=rgb-led EX=ws2812)"; exit 1; }
	cargo run -p $(CRATE) --example $(EX) $(CARGO_FLAGS) -- $(PORT_FLAG)

image: build ## Image flashable autonome (bootloader + partitions + app) dans dist/
	mkdir -p $(DIST)
	espflash save-image --chip $(CHIP) --merge $(ELF) $(DIST)/$(CRATE)-$(PROFILE).bin
	@ls -l $(DIST)/$(CRATE)-$(PROFILE).bin

size: build ## Taille de l'ELF produit
	@ls -l $(ELF)
