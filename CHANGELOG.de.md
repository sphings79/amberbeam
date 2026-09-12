# Änderungen

[English](CHANGELOG.md)

Jede Veröffentlichung bis 1.0 ist eine Vorabversion, und das ist eine Aussage
über den Zustand des Programms, keine Formalie: Es tut, was es sagt, es ist
geprüft, und es ist nicht fertig. Die Update-Benachrichtigung in AmberBeam
liest Vorabversionen genau deshalb.

## 0.1.3

Der Weg zu einem gespeicherten Server, der bisher voraussetzte, dass man weiß,
wo man suchen muss.

### Geändert

- **Verbinden bietet die Server an, die du hast.** Bisher öffnete der Knopf ein
  Formular zum Eintippen einer Adresse — das Einzige, was jemand mit einem
  gespeicherten Server gerade nicht will. Die eigenen Server waren vom Knopf,
  dessen ganzer Zweck das Verbinden ist, nicht erreichbar. Jetzt öffnet er ein
  Menü: die gespeicherten Server zuerst, nach Ordner gruppiert, darunter
  Schnellverbindung und Serverliste. Taste und Knopf öffnen dasselbe Menü an
  derselben Stelle.
- **Die Serverliste hat einen Knopf, wo ein Hauptknopf hingehört** — in der
  Kopfzeile jeder Seite und beschriftet in einer neuen Leiste oben. Bisher saß
  er in der Fußzeile zwischen Darstellung und Hilfe, zwischen lauter Dingen,
  die niemand zweimal die Woche öffnet. Darstellung, Einstellungen und Tasten
  wandern mit nach oben, nach rechts.
- **Das Passwortfeld in den Servereinstellungen ist immer da.** Es erschien
  erst, wenn „merken" angehakt war, und las sich damit wie ein Fehler: Ein
  Feld, das unangekündigt auftaucht, erklärt nicht, warum es vorher nicht da
  war. Der Haken entscheidet jetzt, wie lange ein Passwort gilt, nicht ob man
  überhaupt eines angeben kann.

  Ohne Haken bleibt es für diesen Programmlauf und keinen Moment länger — im
  Arbeitsspeicher, nirgends abgelegt — und das Feld sagt, welche der beiden
  Zusagen gerade gilt. Dauerhaft speichern löscht die Kopie, die nur für diesen
  Lauf gedacht war.
- **Eine einmalige Verbindung in die Serverliste zu übernehmen** war ein Stern
  mit Tooltip. Niemand fährt über ein Symbol, um herauszufinden, ob es das
  gesuchte ist — jetzt steht dran, was es tut.

### Neu

- **Die Update-Prüfung sagt, woran sie ist.** Bisher meldete sie sich nur bei
  Neuigkeiten, Stille bedeutete also dreierlei: noch nicht gefragt, nichts
  Neues, oder GitHub nicht erreichbar. „Aktuell" zu behaupten, weil die Abfrage
  scheiterte, ist eine Vermutung im Gewand einer Antwort — deshalb ist
  „Prüfung fehlgeschlagen" ein eigener Zustand, und ein Klick versucht es
  erneut. Der Klick fragt auch dann, wenn die automatische Prüfung beim Start
  abgeschaltet ist: Diese Einstellung regelt das ungefragte Nachsehen, und ein
  Klick ist nicht ungefragt.

### Behoben

- Der Hinweis unter dem Passwort der Schnellverbindung nannte den
  Schlüsselbund, so heißt der Zugangsdatenspeicher aber nur auf dem Mac, und
  sprach vom „Site Manager", obwohl das Fenster in diesem Programm überall
  „Server" heißt.

## 0.1.2

Unter Windows und Linux war auch 0.1.1 unbenutzbar. Die dort gegebene Erklärung
war falsch; hier steht, woran es wirklich lag.

### Behoben

- **Das Serverfenster ging unter Windows weiterhin weiß auf.** Es wurde aus
  einem synchronen Befehl gebaut, wovor Tauris eigene Dokumentation warnt:
  Unter Windows blockiert das, weil WebView2 den Haupt-Thread braucht und der
  Befehl ihn bereits hält. Das Fenster erschien also, und darin begann nie
  etwas — in 0.1.0 eingefroren, in 0.1.1 weiß.

  Das Fragezeichen im Pfad, dem 0.1.1 das anlastete, war ein echter Fehler und
  richtig behoben, aber nicht dieser.
- **Außerhalb von macOS war jedes Tastenkürzel unerreichbar.** Sie lagen auf
  Meta — auf dem Mac die Befehlstaste, unter Windows die Windows-Taste, unter
  Linux Super. Tasten also, die sich das System nimmt, bevor ein Programm sie
  sieht. In der Statuszeile stand „⌘S" auf einem Rechner, der diese Taste gar
  nicht hat.

  Die Tabelle legt sich nicht mehr fest, welche physische Taste die Befehlstaste
  ist: auf dem Mac ⌘, sonst Strg, entschieden beim Start. Kürzel stehen in den
  Worten der Tastatur, die vor dir liegt, statt in Mac-Symbolen, und die beiden
  Belegungen, die es außerhalb des Macs nicht geben konnte — Vollbild und
  Löschen mit Rückschritt —, haben dort eine Antwort, die funktioniert.

  Selbst gewählte Tasten werden unverändert übernommen.
- **Der Dialog beim ersten Start erklärte Windows-Nutzern Mission Control.** Er
  existiert für ein einziges Problem — auf dem Mac sind F1 bis F12 keine
  Funktionstasten — und erschien trotzdem überall, bis hin zu Knöpfen, die die
  macOS-Systemeinstellungen öffnen wollten. Er kommt jetzt nur noch dort, wo es
  dieses Problem gibt. Sonst startet eine neue Installation direkt mit der
  Belegung der älteren Windows-Programme, und dafür ist dieses Programm da.

### Neu

- Ein Fenster, das nicht starten kann, sagt das. Jeder Fehler vor oder während
  des Starts wird in die Seite geschrieben, und wenn nach acht Sekunden nichts
  gezeichnet wurde, sagt sie auch das. Sein Schweigen hat den Fehler oben
  überhaupt erst eingekreist: Eine Seite, die nie startet, kann nichts melden —
  auch das nicht.

### Geändert

- Die Tastatur-Seite sagt unmissverständlich, dass der macOS-Teil für Windows
  und Linux nicht gilt, und die ausgeschriebenen Kürzel auf den anderen Seiten
  nennen beide Tastaturen.
- Das Wächter-Skript prüft die Tastatur gegen beide Tastaturen statt nur gegen
  die des Rechners, auf dem es läuft. Durch diese Lücke ist das überhaupt
  ausgeliefert worden.

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
