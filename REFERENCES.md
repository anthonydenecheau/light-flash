# Références

## Carte et chaîne d'outils

- ESP32-C3-DevKit-RUST-1, schéma et brochage : https://github.com/esp-rs/esp-rust-board
- esp-rs std-training, origine de `hardware-check`, `rgb-led` et `wifi` :
  https://github.com/esp-rs/std-training
- Livre esp-rs (section `std`) : https://docs.esp-rs.org/book/
- esp-idf-svc : https://github.com/esp-rs/esp-idf-svc
- esp-idf-hal : https://github.com/esp-rs/esp-idf-hal
- Options de build d'esp-idf-sys (`ESP_IDF_VERSION`, `sdkconfig`, composants) :
  https://github.com/esp-rs/esp-idf-sys/blob/master/BUILD-OPTIONS.md
- espflash : https://github.com/esp-rs/espflash

## BLE et provisioning

- esp32-nimble : https://github.com/taks/esp32-nimble
- Exemple de serveur BLE dont dérive `firmware/light-flash/src/main.rs` :
  https://github.com/apollolabsdev/ESP32C3
- Improv Wi-Fi, protocole de provisioning retenu (BACKLOG.md §2.4) :
  https://www.improv-wifi.com/ble/

## Ruban LED

Voir `HARDWARE.md` §10.
