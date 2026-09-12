# Als Container betreiben

[English](container.md)

Dasselbe Programm ohne Fenster: Es läuft auf einer Maschine, und du erreichst
es im Browser. Nützlich, wenn Übertragungen weiterlaufen sollen, nachdem du
den Laptop zuklappst, oder wenn die Dateien dort liegen, wo zuverlässiger
jemand wach ist als an deinem Schreibtisch.

Es ist derselbe Kern, dieselben Befehle, dieselbe Oberfläche. Nur der Weg vom
Fenster zum Kern ändert sich — über HTTP statt durch die Desktop-Schale —, und
die Oberfläche merkt davon nichts.

## Starten

```yaml
services:
  amberbeam:
    image: ghcr.io/sphings79/amberbeam:latest
    container_name: amberbeam
    restart: unless-stopped
    ports:
      - "2122:2122"
    volumes:
      - ./data:/data
      - ./config:/config
    environment:
      AMBERBEAM_PASSWORD: "such dir was aus"
      AMBERBEAM_SECRET_PASSPHRASE: "und noch was anderes"
```

Dann `http://die-maschine:2122` öffnen und mit diesem Passwort anmelden.

## Was man ihm sagt

| | |
|---|---|
| `AMBERBEAM_PASSWORD` | Das eine Passwort. **Ohne es kommt niemand hinein.** |
| `AMBERBEAM_SECRET_PASSPHRASE` | Womit gespeicherte Passwörter verschlüsselt werden. Ohne sie wird keines gespeichert, und das Programm sagt es. |
| `AMBERBEAM_BEHIND_TLS` | Setzen, wenn ein Reverse Proxy davor TLS beendet. |
| `AMBERBEAM_ADDRESS` | Worauf gehört wird. `0.0.0.0:2122`. |
| `AMBERBEAM_CONFIG` | Wo Serverliste und Einstellungen liegen. `/config`. |

Beide Geheimnisse können auch aus einer Datei kommen —
`AMBERBEAM_PASSWORD_FILE` und `AMBERBEAM_SECRET_PASSPHRASE_FILE` —, was du
willst, wenn die Compose-Datei in einem Repository liegt. Die Datei gewinnt
gegen die Variable: Wer ein Secret einhängt, meint es so.

## Die zwei Volumes

`/data` ist die lokale Seite. Dort beginnt die linke Seite, und dort liegen die
Dateien, die du überträgst.

`/config` enthält die Serverliste, die Einstellungen und die verschlüsselte
Passwortdatei. Dieses sichern — das ist der Teil, den du vermissen würdest.

## Dateien hinein und heraus

Zieh eine Datei von deinem Rechner auf die linke Seite, und sie wird in das
Verzeichnis hochgeladen, das dort gerade offen ist. Rechtsklick auf eine Datei
und **Herunterladen** schickt sie den anderen Weg zurück.

Beides gilt nur für `/data`, also die eigenen Dateien dieser Maschine. Ein
Browser kann eine Datei nicht direkt auf einen FTP-Server legen, und er sollte
nicht so tun: Dorthin sind es zwei Schritte, und der zweite ist die
Warteschlange. Ein Drop auf eine Serverseite sagt das, statt auf halbem Weg zu
scheitern.

## Was er nicht tut

**Er spricht selbst kein TLS.** Stell einen Reverse Proxy davor, der auf dieser
Maschine ziemlich sicher ohnehin schon läuft. Ein Proxy erneuert Zertifikate,
was ein Container nicht kann, und er tut es an einer Stelle statt einmal pro
Programm. Setz `AMBERBEAM_BEHIND_TLS`, damit das Sitzungs-Cookie entsprechend
markiert wird.

**`/data` ist ein Startpunkt, kein Zaun.** Nichts hindert jemanden, der
angemeldet ist, daran, aus dem Verzeichnis heraus in den Rest des Containers zu
gehen. Dort oben ist nicht viel — das Image enthält das Programm und wenig
sonst, und es läuft nicht als root —, aber das sollte man wissen, statt etwas
anderes anzunehmen.

**Ein Passwort, keine Benutzerkonten.** Das ist ein Programm, das man für sich
selbst betreibt. Eine Benutzertabelle hieße Anmeldeseite, Passwort-Zurücksetzen
und eine Migration, alles für eine Person, der die Maschine ohnehin gehört.

**Alle teilen sich eine Sitzung.** Zwei Browser sehen dieselbe Warteschlange,
dieselben Verbindungen und dieselbe Serverliste, weil ein Programm läuft und
beide daraufschauen. Das ist der Sinn und keine Einschränkung: So startest du
etwas auf einer Maschine und siehst von einer anderen aus zu.

## Die Passwortdatei

Gespeicherte Passwörter liegen in `/config/secrets.sealed`, verschlüsselt unter
der Passphrase mit PBKDF2-HMAC-SHA256 über 600 000 Runden und
ChaCha20-Poly1305 — dieselbe Versiegelung, die ein Export mit Passwörtern
benutzt.

**Eine falsche Passphrase hält das Programm an.** Es startet nicht mit einem
leeren Speicher weiter, denn das erste danach gespeicherte Passwort würde alles
überschreiben, was schon in der Datei steht. Ein Tippfehler darf nicht
zerstören, was er nicht lesen konnte.

Geht die Passphrase verloren, sind die gespeicherten Passwörter weg. Es gibt
keinen Weg zurück in diese Datei, und genau das heißt „verschlüsselt".

## Vom Desktop-Programm aus erreichen

Das Desktop-Programm kann statt des eigenen Kerns einen laufenden Dienst
bedienen: dasselbe Fenster, dieselben Tasten, aber die Übertragungen passieren
drüben. Unter **Server** wählst du zwischen diesem Rechner und einem Dienst,
gibst Adresse und Passwort an, und das Fenster schaltet um.

Siehe auch: [Wo die Geheimnisse liegen](security.de.md).
