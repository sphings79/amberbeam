# Wo die Geheimnisse liegen

[English](security.md)

Kurzfassung: im Zugangsdatenspeicher des Systems, und sonst nirgends.

## Passwörter

Ein Servereintrag ist eine lesbare JSON-Datei unter
`~/Library/Application Support/AmberBeam/sites/` — eine je Server, in Ordnern,
die echte Ordner sind. Du kannst sie lesen, von Hand ändern, sichern oder in
eine Versionsverwaltung legen.

Das ist nur deshalb vertretbar, weil **kein Passwort darin steht**. Jeder
Eintrag verweist über eine Kennung auf seine Geheimnisse, und das Passwort
selbst liegt im Schlüsselbund unter macOS, in der
Anmeldeinformationsverwaltung unter Windows oder im Secret Service unter
Linux. Ob ein Passwort überhaupt gemerkt wird, entscheidest du pro Eintrag;
Abschalten löscht das Gespeicherte.

Ein Passwort erreicht das Fenster des Programms nie. Die Verbindung wird
darunter aufgebaut, wo das Passwort in genau diesem Moment geholt wird — es
gibt also keinen Zeitpunkt, zu dem es in einer Oberfläche liegt und darauf
wartet, ausgelesen zu werden.

## Serverschlüssel und Zertifikate

**SSH-Host-Keys** werden gegen `~/.ssh/known_hosts` geprüft — dieselbe Datei,
die auch das Terminal benutzt. Ein Server, den noch niemand gesehen hat, fragt
einmal und zeigt seinen Fingerabdruck. Ein Server, dessen Schlüssel sich
*geändert* hat, wird abgewiesen, denn genau so sieht ein Lauschangriff aus.

**TLS-Zertifikate** werden gegen die geprüft, denen dieser Rechner ohnehin
traut, einschließlich derer, die deine Firma eingetragen hat. Ein Zertifikat,
für das niemand bürgt, wird abgewiesen, und der Grund wird beim Namen genannt
statt zu „Zertifikatsfehler" zusammengefasst: selbstsigniert, abgelaufen, noch
nicht gültig, falscher Name, zurückgezogen oder Signatur kaputt. Annehmen gilt
für **genau dieses Zertifikat auf genau diesem Host und Port** — bei einem
anderen wird erneut gefragt — und der Bereich trägt eine Kennzeichnung,
solange die Ausnahme gilt.

Ausnahmen stehen offen in `accepted-certificates.json` daneben, damit du sie
ohne Hilfe dieses Programms sehen und entfernen kannst.

## Was nicht verschlüsselt ist, und das sagt

Reines FTP überträgt Passwort und jede Datei im Klartext. AmberBeam tut es,
weil manche Server nichts anderes anbieten, und kennzeichnet die Verbindung
rot, solange sie offen ist.

FTPS verschlüsselt die Steuerverbindung **und** die Daten. Ein Mittelweg
existiert nicht: Weigert sich ein Server, den Datenkanal zu verschlüsseln,
scheitert die Verbindung und sagt, welches von beidem er verweigert hat. Ein
Datenkanal ohne `PROT P` ist nicht halb sicher, er ist im Klartext.

## Exporte

Ohne Passwörter ist ein Export reines JSON. Mit ihnen ist er unter einem
Kennwort verschlossen, das du wählst — PBKDF2 und ChaCha20-Poly1305, die
gewöhnliche Konstruktion —, und das Kennwort wird nirgends gespeichert.
Verloren heißt, die Datei ist mit verloren. Ein Drittes gibt es nicht.

## Was AmberBeam nie tut

Keine Konten, keine Telemetrie, keine eigenen Server. Nichts darüber, womit du
dich verbindest, verlässt deinen Rechner.
