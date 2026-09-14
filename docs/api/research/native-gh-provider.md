# Nativer GitHub-Provider über `gh`

## Ziel

R6 legt einen direkten GitHub-Provider fest, ohne Repository-, Host- oder
Authentifizierungseinstellungen von `gh` zu duplizieren. Ein fehlgeschlagener Remote-Versuch muss
den Report lokal sichern und den Remote-Fehler sichtbar lassen.

## Verhalten von `gh`

`gh issue create` nimmt Titel und Body nicht interaktiv über `--title` und `--body-file` entgegen.
`--body-file -` liest den Body von stdin. Ohne `--repo` verwendet `gh` seinen nativen
Repository-Kontext. Die von `gh` unterstützte Umgebung kann diesen Kontext über `GH_REPO` und
`GH_HOST` beeinflussen. Authentifizierung bleibt ebenfalls bei `gh`.

Quellen:

- [`gh issue create`](https://cli.github.com/manual/gh_issue_create)
- [`gh issue list`](https://cli.github.com/manual/gh_issue_list)
- [`gh`-Umgebungsvariablen](https://cli.github.com/manual/gh_help_environment)
- [`gh api` und Pagination](https://cli.github.com/manual/gh_api)

Der Provider startet alle `gh`-Prozesse im Projektordner der Controlled Session. Er übergibt kein
`--repo` und setzt keine Repository-, Host- oder Token-Variable. Er darf interaktive Prompts
abschalten, damit fehlende Anmeldung als Fehler zurückkehrt und den lokalen Rückfall auslöst.

## Direkter Ablauf

Bei `provider::Config::Github` führt `report::submit` diese Schritte aus:

Die wirksame Provider-Konfiguration stammt aus der laufenden Session. `report::submit` nimmt keine
zweite Konfiguration entgegen.

1. Den Issue-Titel unverändert aus `Report::title` übernehmen.
2. Den Markdown-Body aus dem vollständigen Report erzeugen und den Signatur-Marker ergänzen.
3. Über `gh` offene und geschlossene Issues nach dem exakten Marker durchsuchen.
4. Bei einem Treffer `Existing` mit Issue-Nummer und URL zurückgeben, ohne das Issue zu verändern.
5. Ohne Treffer das Issue direkt über `gh issue create` veröffentlichen.
6. Bei jedem Fehler der Remote-Schritte den lokalen Markdown-Provider mit demselben Report
   ausführen.
7. Bei erfolgreichem lokalen Rückfall `Fallback` mit lokalem Pfad und Remote-Fehler zurückgeben.
8. Schlägt auch das lokale Schreiben fehl, einen Fehler mit beiden Ursachen zurückgeben. R7 legt
   dessen öffentliche Form fest.

Eine erfolgreiche Duplikatsuche oder Veröffentlichung erzeugt keine lokale Markdown-Datei. Der
lokale Report entsteht nur bei `provider::Config::Local` oder als Rückfall nach einem Remote-Fehler.

## Signatur-Marker

Der Body enthält eine eigene Zeile dieser Form:

```markdown
<!-- bug_hunter-signature: v1:sha256:<64 kleingeschriebene Hex-Zeichen> -->
```

Der Provider erzeugt den Marker ausschließlich aus `Report::signature`. Titel, übriger Body,
Zeitangaben und Report-Context werden nicht verglichen. Ein Kandidat gilt nur dann als Duplikat,
wenn sein Body den vollständigen Marker exakt enthält.

Die Suche umfasst offene und geschlossene Issues und muss alle Ergebnisse paginieren. Eine Suche,
die nur die ersten 1.000 Issues oder nur Titel vergleicht, erfüllt den Vertrag nicht. `gh api
--paginate` stellt die dafür nötige Pagination bereit.

GitHub bietet beim Erstellen eines Issues keine atomare Eindeutigkeitsbedingung für einen
benutzerdefinierten Body-Marker. Zwei gleichzeitig laufende Veröffentlichungen können deshalb nach
beiderseits erfolgloser Suche zwei Issues erstellen. Der Provider verhindert normale sequenzielle
Duplikate, verspricht aber keine globale Nebenläufigkeitsgarantie.

## Konfiguration

`github::Config` besitzt in diesem Umbau keine Felder. Die Wahl
`provider::Config::Github(github::Config {})` ist die ausdrückliche Entscheidung für die direkte
Remote-Veröffentlichung.

Folgende Optionen gehören nicht zu R6:

- Repository- oder Host-Overrides,
- Tokens oder andere Authentifizierungsdaten,
- Labels, Assignees und Milestones,
- GitHub-spezifische Titelpräfixe,
- Kommentare oder erneutes Öffnen bestehender Issues.

Spätere Erweiterungen brauchen eigene Entscheidungen und Tests. Sie dürfen die native
`gh`-Auflösung nicht stillschweigend verändern.

## Abweichungen vom aktuellen Code

Der aktuelle Ablauf schreibt vor jedem Veröffentlichungsversuch einen lokalen Entwurf und sucht nur
unter höchstens 1.000 Issues nach einem identischen Titel. Das Ziel schreibt bei erfolgreichem
GitHub-Ablauf keine lokale Datei und verwendet ausschließlich den vollständigen Signatur-Marker.

Der aktuelle `gh`-Aufruf besitzt bereits keine eigenen Token- oder Repository-Felder. Diese
Eigenschaft bleibt erhalten.
