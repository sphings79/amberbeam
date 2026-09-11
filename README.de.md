<div align="center">

<img src="assets/icon.svg" width="96" height="96" alt="AmberBeam Logo">

# AmberBeam — Zweifenster-Client für FTP und SFTP auf macOS, Windows und Linux

**Ein Dateiübertragungsprogramm in der Tradition von FlashFXP.** Server-Log
oben, zwei Dateibereiche mit Ordnerbaum in der Mitte, Warteschlange unten — und
die Tastatur führt. Als Programm auf macOS, Windows und Linux, und als Container
mit Weboberfläche für den eigenen Server.

[![Lizenz: AGPL v3](https://img.shields.io/badge/Lizenz-AGPL--3.0-e08b12?style=flat-square)](LICENSE)
[![Plattformen](https://img.shields.io/badge/Plattformen-macOS%20(Apple%20Silicon)%20%7C%20Windows%20%7C%20Linux%20%7C%20Docker-2b3040?style=flat-square)](#selbst-bauen)
[![Kern in Rust](https://img.shields.io/badge/Kern-Rust-b7410e?style=flat-square)](https://www.rust-lang.org/)
[![Oberfläche in Svelte](https://img.shields.io/badge/Oberfl%C3%A4che-Svelte%205-ff3e00?style=flat-square)](https://svelte.dev/)
[![CI](https://img.shields.io/github/actions/workflow/status/sphings79/amberbeam/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/sphings79/amberbeam/actions/workflows/ci.yml)
[![Sterne](https://img.shields.io/github/stars/sphings79/amberbeam?style=flat-square&color=f0b429)](https://github.com/sphings79/amberbeam/stargazers)

[English version](README.md) · [Stand](#stand) · [Was es werden soll](#was-es-werden-soll) · [Die drei Nähte](#die-drei-nähte) · [Selbst bauen](#selbst-bauen) · [Meilensteine](#meilensteine) · [Übersetzen](#übersetzen) · [Lizenz](#lizenz)

**Vom selben Autor:** [AmberChest — IMAP-Postfächer als einfache .eml-Dateien sichern](https://github.com/sphings79/amberchest)

</div>

---

## Stand

> **Meilenstein M0: Das Gerüst steht, übertragen wird noch nichts.**
> Die Werkzeugkette ist bewiesen — die CI baut Programm und Installationspaket
> bei jedem Push auf macOS, Windows und Linux — und die drei Architekturnähte,
> die nachträglich teuer wären, sind angelegt. SFTP kommt mit M1, Übertragungen
> mit M2. Die [Meilensteine](#meilensteine) sagen, was fertig ist und was nicht;
> dieses Dokument behauptet nichts anderes.

## Warum noch ein Übertragungsprogramm?

Transmit und ForkLift sind gute Programme. Sie sind aber durch und durch
Mac-Programme. Was Umsteigern von Windows fehlt, ist keine Funktionsliste,
sondern eine Reihe von **Handgriffen**: die Funktionstasten, der Fokuswechsel
per Taste, der Ordnerbaum neben jeder Liste, die Warteschlange unten, der rohe
Server-Log oben. Für dieses Muskelgedächtnis gibt es bisher keinen Platz.

Genau diese Lücke besetzt AmberBeam. Wo Plattform-Konvention und
FlashFXP-Gewohnheit kollidieren, **gewinnt die Gewohnheit** — solange das
Programm dadurch nicht gegen das Betriebssystem kämpft. Wo es das täte, wirst du
beim ersten Start gefragt.

Dieselbe Überlegung trägt es nach Linux, wo es ebenfalls nichts Vergleichbares
gibt, und zurück nach Windows, wo das Vorbild seit Jahren stillsteht.

## Bilder

<div align="center">

<img src="assets/screenshots/layout.svg" width="880" alt="Fensterlayout von AmberBeam: Server-Log oben, lokale Dateien mit Ordnerbaum links, Server rechts, Warteschlange unten, dunkel und hell nebeneinander">

<em><strong>Das geplante Fenster</strong>, dunkel und hell in der Mitte
auseinandergerissen. Gezeichnet, nicht fotografiert: Die Bereiche listen noch
keine Dateien — das ist M1. Alles darauf ist entschieden, nichts davon ist fürs
Bild erfunden.</em>

</div>

## Was es werden soll

Fassung 1, wie festgelegt:

- **SFTP, FTP und FTPS**, voreingestellt explizit über `AUTH TLS`, implizit auf
  Port 990 unterstützt, unverschlüsseltes FTP möglich, aber sichtbar markiert
- **Zwei Bereiche mit je Ordnerbaum und Dateiliste.** Jede Seite kann lokal
  *oder* Server sein — zwei Server nebeneinander sind ausdrücklich erlaubt
- **Warteschlange, die Programmneustarts übersteht**, mit Parallelität pro
  Server (SFTP 8, FTP 4 voreingestellt, bis 64 einstellbar)
- **Fortsetzen einer einzelnen Datei, die mittendrin abgerissen ist** — nicht
  nur der Warteschlange. Größe und Zeitstempel der Quelle werden vorher
  verglichen; bei Abweichung wird gefragt, statt stillschweigend eine Datei aus
  zwei Fassungen zusammenzusetzen
- **Site Manager mit Import** aus FileZilla, WinSCP, `~/.ssh/config`, Total
  Commander und FlashFXP — niemand tippt dreißig Server neu ein
- **PuTTY-Schlüssel (`.ppk`)** werden gelesen und umgewandelt. Das kann kein
  Mac-Client, und jeder Windows-Umsteiger braucht es
- **Funktionstasten wie in FlashFXP**, frei belegbar. Auf macOS, wo F1 bis F12
  standardmäßig dem System gehören, bietet ein Dialog beim ersten Start drei
  Wege an
- **Zugangsdaten im Speicher des Systems** — Schlüsselbund auf macOS,
  Anmeldeinformationsverwaltung auf Windows, Secret Service auf Linux. Nie in
  einer Konfigurationsdatei
- **Deutsch und Englisch**, hell, dunkel und Systemvorgabe, fünf Akzentfarben
- **Keine Cloud, keine Konten, keine Telemetrie**

Später: Verzeichnisabgleich, Vergleich beider Seiten, entferntes Bearbeiten,
zeitgesteuerte Aufträge, FXP, die Container-Fassung, S3 und WebDAV.

## Die drei Nähte

Drei Entscheidungen mussten vor der ersten Zeile Code stimmen, weil sie
nachträglich jede einzelne Aufrufstelle betreffen:

| # | Naht | Was sie bedeutet |
|---|------|------------------|
| 08 | **Quelle und Ziel sind Endpunkte** | Die Übertragungseinheit kennt zwei beliebige Endpunkte und nimmt nirgends an, dass eine Seite die lokale ist. Server zu Server und später FXP sind zusätzliche Fälle statt eines Umbaus am Fundament |
| 09 | **Eine Tür zum Kern** | Die Oberfläche ruft niemals Tauri auf. Alles läuft durch `src/lib/bridge` mit zwei Umsetzungen: Tauris Kanal und HTTP samt WebSocket für die Container-Fassung |
| 11 | **Kein Text im Code** | Jede Zeichenkette läuft durch `t()` und steht in einer flachen JSON-Sprachdatei. Englisch ist die Rückfallebene, damit eine unfertige Übersetzung nie einen rohen Schlüssel zeigt |

Naht 2 und 3 werden bei jedem Bauvorgang von `scripts/check-seams.mjs` und
`scripts/check-lang.mjs` geprüft — eine Regel, die niemand durchsetzt, gilt nur
so lange, wie alle sie erinnern.

## Selbst bauen

Nötig sind auf jeder Plattform [Rust](https://rustup.rs) und Node ab 22, dazu
das, was dein System zum Bauen eines nativen Fensters braucht:

| | Zusätzlich |
|---|---|
| **macOS 13+, Apple Silicon** | `xcode-select --install` |
| **Windows** | Microsoft C++ Build Tools und die WebView2-Laufzeit (bei Windows 11 dabei) |
| **Linux** | `sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-3-dev` |

Danach überall gleich:

```sh
git clone https://github.com/sphings79/amberbeam.git
cd amberbeam
npm install
npm run tauri dev     # Entwicklungsfenster mit Neuladen
npm run tauri build   # Programm und Installationspaket in target/release/bundle
```

Heraus kommen `.app` und `.dmg` auf macOS, `.msi` und ein NSIS-Installer auf
Windows, `.deb`, `.rpm` und ein AppImage auf Linux.

> **Auf dem Mac heißt das Apple Silicon, und nur Apple Silicon.** Keine
> Intel-Fassung, kein Universalpaket: macOS 26 Tahoe ist die letzte Fassung, die
> Intel überhaupt noch trägt, und wer sich heute einen Mac kauft, um von Windows
> wegzukommen, kauft MacBook Air oder Mac mini — beide seit Generationen
> ausschließlich Apple Silicon. Rust und Tauri sind architekturunabhängig, eine
> Intel-Fassung wäre eine Zeile in der Baueinstellung. Die Tür bleibt offen, wir
> gehen nur nicht hindurch.

Der erste `cargo build` lädt viel herunter und dauert mehrere Minuten. Das ist
normal und kein hängender Vorgang.

> **Ehrlichkeitshalber:** Nur die macOS-Fassung läuft auf dem Rechner des
> Autors. Windows und Linux baut die CI bei jedem Push — das beweist, dass sie
> übersetzen und sich paketieren lassen, nicht dass sie sich richtig anfühlen.
> Fehlermeldungen von diesen beiden sind willkommen und werden ernst genommen.

Die macOS-`.dmg` ist **ad-hoc signiert, nicht beglaubigt**, der Windows-Installer
ist unsigniert — dahinter stehen weder eine bezahlte Apple-Mitgliedschaft noch
ein Code-Signing-Zertifikat. Auf dem eigenen Rechner genügt das; anderswo melden
sich Gatekeeper und SmartScreen zu Wort.

### Prüfungen

```sh
npm run verify                    # Sprachdateien, Nähte, Typen
cargo test --package amberbeam-core
cargo clippy --package amberbeam-core --all-targets -- -D warnings
```

Der Kern ist ein eigenes Crate, das nichts von Tauri weiß, und baut und testet
daher auf jedem System — mit Absicht, denn die Container-Fassung aus M7 hängt
daran. Die CI erzwingt das bei jedem Push.

### Bilder neu erzeugen

```sh
python3 dev/make-screenshots.py
dev/render-png.py assets/social-preview.svg assets/social-preview.png 1280 640
dev/render-png.py assets/icon.svg assets/icon.png 1024 1024
npm run tauri icon assets/icon.png
```

`render-png.py` nutzt QuickLook und `sips` und braucht daher einen Mac. Die SVGs
selbst sind die Quelle und überall lesbar.

## Meilensteine

| | Meilenstein | Stand |
|---|---|---|
| **M0** | Gerüst: ein Fenster, Installationspakete auf drei Plattformen, die drei Nähte, Lizenz und README | **fertig** |
| **M1** | SFTP: mit Passwort und Schlüssel verbinden, Verzeichnisse auflisten, Baum und Liste, zwei Bereiche, Fokuswechsel | als Nächstes |
| **M2** | Übertragen: Warteschlange, Parallelität, Fortsetzen, Konflikte, Fortschritt. Ab hier benutzbar | |
| **M3** | FTP und FTPS: Kontroll- und Datenkanal, `MLSD` bevorzugt, `LIST` mit Dialekterkennung als Rückfallebene | |
| **M4** | Site Manager: Zugangsdatenspeicher des Systems, Import aus FileZilla, WinSCP, OpenSSH und FlashFXP, Export | |
| **M5** | Tastatur und Einstellungen: F-Tasten, Einrichtungsdialog, frei belegbare Schemata, Raw-Befehle, Serversuche | |
| **M6** | Feinschliff: Symbol, Signierung und Beglaubigung, Hilfe, erste Veröffentlichung | |
| **M7** | Container: derselbe Kern hinter HTTP und WebSocket, ein Benutzer, Docker-Abbild für amd64 und arm64 — Übertragungen zwischen zwei entfernten Servern laufen dann dort statt durch deine Hausleitung | |

## Übersetzen

Sprachen sind flache JSON-Dateien unter `src/lib/i18n/`. `en.json` kopieren,
Werte übersetzen, Schlüssel behalten, Pull Request schicken — oder ab M5 die
Datei in den Sprachordner des Programms legen und neu starten. Kein Bauvorgang,
keine Werkzeugkette.

`npm run check:lang` meldet, was fehlt oder zu viel ist.

## Lizenz

**AGPL-3.0-or-later.** Die Affero-Klausel steht hier aus einem Grund: der
Container-Fassung. Wer sie nimmt, verändert und als bezahlten Dienst ins Netz
stellt, muss seine Änderungen offenlegen. Für das Programm auf dem Schreibtisch
ist der Unterschied null — ein Schreibtischprogramm bietet niemandem
Netzinteraktion an.

---

<div align="center">

**Wenn du das fertig sehen willst, hilft ein Stern.** So findet es der Nächste,
dem FlashFXP fehlt.

[![Buy me a coffee](https://img.shields.io/badge/Buy%20me%20a%20coffee-sphings-f0b429?style=flat-square&logo=buymeacoffee&logoColor=black)](https://buymeacoffee.com/sphings)

</div>
