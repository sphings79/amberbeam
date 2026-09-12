# Änderungen

[English](CHANGELOG.md)

Jede Veröffentlichung bis 1.0 ist eine Vorabversion, und das ist eine Aussage
über den Zustand des Programms, keine Formalie: Es tut, was es sagt, es ist
geprüft, und es ist nicht fertig. Die Update-Benachrichtigung in AmberBeam
liest Vorabversionen genau deshalb.

## 0.1.1

Unter Windows war 0.1.0 unbenutzbar, und das ist der Grund.

### Behoben

- **Das Serverfenster ging leer auf, fror ein und riss den Rest mit.** Einem
  Fenster wird ein *Pfad* zum Laden gegeben, und der Pfad lautete
  `index.html?view=sites`. Ein Fragezeichen ist in einem Pfad unter macOS ein
  gewöhnliches Zeichen und unter Windows ein verbotenes — das Fenster hatte
  also nichts zu laden. Welche Ansicht ein Fenster zeigt, entscheidet jetzt
  seine Beschriftung.

  Dieser eine Fehler hat alle drei gemeldeten Erscheinungen verursacht. Die
  Antwort jedes Befehls läuft über denselben Faden zurück ins Fenster, den das
  eingefrorene Fenster blockiert hatte — deshalb blieb der Einstellungsdialog
  leer, und „verbindet …" stand still, obwohl die Verbindung längst stand.
- **Verbinden konnte ewig warten**, und das war nie ein Windows-Problem. Ein
  Server, der die Verbindung annimmt und dann schweigt — eine Firewall, die
  verschluckt statt abzulehnen; ein Port, hinter dem ein anderes Programm sitzt
  — wurde abgewartet, bis jemand aufgab. Zwanzig Sekunden decken jetzt das
  ganze Hallo ab, und die Meldung sagt, dass da *etwas* geantwortet hat: genau
  das schickt dich zur Firewall statt zur Adresse.
- **Serverliste und Importsuche** laufen nicht mehr auf dem Faden, der zeichnet.
  Die eine fragt den Zugangsdatenspeicher je Eintrag, die andere läuft
  Downloads, Schreibtisch und Dokumente zwei Ebenen tief ab.

### Geändert

- Das macOS-Abbild verlangt beim Öffnen keine Zustimmung zur ganzen AGPL mehr.
  Die Lizenz regelt die Weitergabe, nicht die Benutzung.
- Das Linux-Paket legt sein 256-Pixel-Symbol unter `256x256` ab statt unter
  `256x256@2` — ein Verzeichnisname, den keine Oberfläche kennt.

## 0.1.0

Der erste öffentliche Stand. Alles unten funktioniert und ist benutzt worden;
was fehlt, steht am Ende.

### Übertragen

- **SFTP, FTPS und FTP.** Explizites `AUTH TLS` als Voreinstellung, implizit auf
  Port 990, und reines FTP, wo ein Server nichts anderes anbietet — rot
  gekennzeichnet, solange es offen ist.
- **Mehrere Dateien gleichzeitig**, pro Server einstellbar. Acht über SFTP, vier
  über FTP — und lehnt ein Server eine weitere Anmeldung ab, senkt AmberBeam den
  Wert selbst und sagt es, statt den Rest der Warteschlange in Fehlern enden zu
  lassen.
- **Eine Warteschlange, die den Neustart übersteht.** Programm mitten in einer
  Übertragung schließen — der Rest ist beim nächsten Start noch da.
- **Abgerissene Übertragungen machen weiter**, wo sie aufgehört haben — aber nur,
  wenn das andere Ende noch dieselbe Größe und dasselbe Alter hat. Hat es sich
  geändert, wird neu begonnen, statt zwei Fassungen zusammenzunähen.
- **Bei Konflikten wird gefragt**, mit „für die übrigen genauso" im selben
  Dialog.
- Dateien zwischen den Bereichen ziehen, oder aus dem Finder herein.

### Server

- **Eine Serverliste in Ordnern**, eine lesbare JSON-Datei je Eintrag, und in
  keiner davon ein Passwort: die gehen in den Schlüsselbund, die
  Anmeldeinformationsverwaltung oder den Secret Service.
- **Import aus fünf anderen Programmen**: FileZilla, WinSCP, Total Commander,
  OpenSSH sowie `Sites.dat` oder der `.ftp`-Export eines älteren
  Windows-Clients. Ordner kommen mit.
- **Export** als reines JSON ohne Passwörter, oder mit ihnen unter einem
  Kennwort verschlossen. Ein Drittes gibt es nicht.
- Schlüsseldateien einschließlich PuTTYs `.ppk`, beide Fassungen, verschlüsselt
  oder nicht.

### Prüfen, wer am anderen Ende ist

- **SSH-Host-Keys** gegen `~/.ssh/known_hosts`. Ein geänderter Schlüssel wird
  abgewiesen.
- **TLS-Zertifikate** gegen den Vertrauensspeicher des Systems. Was nicht stimmt,
  wird beim Namen genannt — selbstsigniert, abgelaufen, noch nicht gültig,
  falscher Name, zurückgezogen, Signatur kaputt — statt zu „Zertifikatsfehler"
  zusammengefasst. Annehmen gilt für ein Zertifikat auf einem Host und Port.
- FTPS verschlüsselt auch den Datenkanal, oder die Verbindung scheitert und sagt,
  welche Hälfte der Server verweigert hat.

### Bedienung

- Drei Tastaturbelegungen, jede Taste änderbar, und ein Dialog beim ersten
  Start, der herausfindet, welche der F-Tasten-Hürden auf dem Mac bei dir im Weg
  steht — indem er dich eine Taste drücken lässt.
- Liste beim Tippen filtern; Unterordner auf Verlangen durchsuchen, mit
  hinterher ausgewiesenen Kosten.
- Raw-FTP-Befehle unter dem Log.
- Hell, dunkel und nach Systemvorgabe; fünf Akzentfarben; Deutsch und Englisch.

### Noch nicht dabei

- Kein FXP: Übertragungen zwischen zwei Servern laufen noch über diesen Rechner.
- Kein Verzeichnisvergleich, kein Spiegeln.
- Kein entferntes Bearbeiten.
- Keine zeitgesteuerten Aufträge.
- Die Container-Fassung mit Weboberfläche kommt noch.
- Nur macOS ist wirklich im Einsatz. Unter Linux ist das Debian-Paket in ein
  frisches Debian 12 installiert und geprüft worden: Abhängigkeiten stimmen,
  nichts fehlt, und Programm, Starteintrag und Symbole landen dort, wo sie
  hingehören — aber das Fenster hat noch niemand auf einem echten Linux-Rechner
  aufgehen sehen. Windows baut die CI, sonst ist es ungeprüft.
