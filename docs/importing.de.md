# Server mitbringen

[English](importing.md)

Dreißig Server tippt niemand neu ein. AmberBeam liest die Listen von sechs
anderen Programmen — Serverliste öffnen (**⌘S**) und auf Importieren.

Es sucht selbst an den üblichen Orten und zeigt, was es gefunden hat. Eine
Datei, die woanders liegt, lässt sich wählen oder einfach auf das Fenster
ziehen.

## Was gelesen wird

| Programm | Datei | Passwörter |
|---|---|---|
| OpenSSH | `~/.ssh/config` | keine zu lesen — da stehen keine drin |
| FileZilla | `sitemanager.xml` | ja, außer bei gesetztem Hauptpasswort |
| WinSCP | `WinSCP.ini` | ja, außer bei gesetztem Hauptpasswort |
| Total Commander | `wcx_ftp.ini` | ja |
| Ein älterer Windows-Client | `Sites.dat` | so gut es geht |
| Derselbe, exportiert | `.ftp`-Datei | ja — die stehen dort im Klartext |

Ordner kommen mit. Ein Baum aus FileZilla oder WinSCP kommt hier als Baum an.

## Zu diesen Passwörtern

In den meisten dieser Dateien sind die Passwörter **verschleiert, nicht
verschlüsselt**. Die Verfahren sind seit Jahren veröffentlicht; wer die Datei
hat, kann sie lesen. Genau deshalb lohnt es, sie zu übernehmen: Sobald
AmberBeam sie hat, liegen sie im Zugangsdatenspeicher dieses Rechners und in
keiner Datei mehr.

Du wirst vorher gefragt, und es steht dabei, was das heißt. Geschrieben wird
nichts, bevor du die Liste gesehen und angekreuzt hast.

Wo ein Passwort nicht lesbar ist — ein Hauptpasswort in der Quelle, oder eine
Datei aus einer Fassung mit anderem Verfahren —, kommt der Eintrag **ohne**
an statt mit einem falschen, und der Import sagt das vorher. Ein Passwort, das
sich stillschweigend nicht anmeldet, ist schlimmer als eines, das nie
behauptet wurde.

Wenn du aus einem `.ftp`-Export importiert hast: lösch die Datei danach. Darin
stehen alle deine Passwörter im Klartext.

## Wieder herausnehmen

Dasselbe Fenster exportiert: reines JSON ohne Passwörter, oder mit ihnen unter
einem Kennwort verschlossen. Ein Drittes gibt es nicht. Eine Datei mit
Passwörtern, die nur verschleiert ist, ist genau das, womit die Importer oben
ihre Zeit verbringen.
