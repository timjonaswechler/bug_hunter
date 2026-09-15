   # bug_hunter API-Zielplanung

   ## Zweck

   Wir entwerfen vor der Neuimplementierung das Ziel-Interface und die geplante Modulstruktur von
   `bug_hunter`.

   Die Planung soll eine klare Richtung vorgeben, ohne die spätere Implementierung bereits
   kleinteilig festzuschreiben. `goal.rs` ist deshalb Rust-ähnlicher Pseudocode und muss nicht
   kompilieren.

   ## Aktueller Planungsstand

   Für den derzeit bekannten Stand ausreichend skizziert:

   - `command::input`
   - `command::tick`
   - `command::screenshot`
   - `command::inspect`
   - `command::recording`
   - `command::replay`
   - `report`
   - `session::protocol`
   - `session::launch`
   - `session::Session`
   - Controlled-Session-Integration mit `session::Plugin`
   - `cli::repl`
   - `cli::script`

   Noch zu planen:

   - weitere Start- und Lebensdauerregeln eines persistenten lokalen Servers für mehrere unabhängige Sessions
   - Session-Verwaltung mit Identität, Erzeugen, Auflisten, Auswahl und Zustandsabfrage
   - lokaler Client-Transport und Zugriffsschutz
   - Session-Zuordnung und Cursor-Regeln des gemeinsamen Activity-Vertrags
   - vollständige CLI-Bedienung sowie Client-Zugriff für REPL, Script und Agent
   - getrennte Regeln für Session-Shutdown und Serverende
   - Einstiegspunkte, öffentliche Exports, Feature- und Crate-Zuschnitt

   Beschlossen ist ein lokaler Server für mehrere Sessions, siehe
   [`ADR-0023`](../adr/0023-local-server-manages-multiple-sessions.md).
   Weboberflächen und weitere UIs sind nicht Teil des aktuellen Umfangs; MCP entfällt.
   Die Agent-Laufzeit und ihre Modellanbindung sind noch nicht festgelegt.

   [`ADR-0024`](../adr/0024-separate-server-client-and-cli.md) trennt `server` für die
   Bereitstellung, `client` für den Zugriff und `cli` für die Bedienung. Das Sammelmodul
   `host` entfällt. Die Modulplatzierung des Agent-Zugangs bleibt offen.

   "Ausreichend skizziert" bedeutet nicht endgültig abgeschlossen. Neue Erkenntnisse dürfen einen
   Bereich wieder öffnen.

   ## Dokumente

   - [`goal.rs`](goal.rs) zeigt die geplante Modulstruktur und das Ziel-Interface.
   - [`current.md`](current.md) beschreibt ausschließlich den aktuellen Code.
   - [`migration.md`](migration.md) hält getroffene Entscheidungen und den Übergang vom Ist-Stand zum
     Ziel fest.
   - [`decisions.md`](decisions.md) verweist auf angenommene projektweite ADRs.
   - [`research/`](research/) enthält technische Untersuchungen, auf denen Entscheidungen beruhen.

   ## Arbeitsablauf

   ### 1. Einen Bereich auswählen

   Wir bearbeiten jeweils einen fachlichen Bereich, zum Beispiel `command::screenshot`. Vor der
   Planung werden der aktuelle Eintrag in `current.md`, der bestehende Code und bereits getroffene
   Entscheidungen in `migration.md` gelesen.

   ### 2. Das Ziel grob skizzieren

   In `goal.rs` werden nur Elemente aufgenommen, die zum Verständnis des Moduls notwendig sind:

   - die Modulstruktur,
   - zentrale Structs, Enums und Traits,
   - wichtige Commands und Outputs,
   - wichtige Funktionen, an denen die Nutzung erkennbar wird,
   - kurze Kommentare zu Regeln, die ein Aufrufer kennen muss.

   Private Hilfsfunktionen, vollständige Fehlerlisten, genaue Validierungsabläufe und
   Implementierungsdetails bleiben zunächst draußen.

   ### 3. Offene Fragen sichtbar machen

   Ungeklärte Fragen werden in `goal.rs` mit `OPEN` markiert. `OPEN` wird nur für echte
   Entscheidungen verwendet, nicht als Ersatz für fehlende Recherche.

   Wenn eine technische Aussage unsicher ist, wird sie anhand der aktuellen Abhängigkeiten und
   möglichst anhand primärer Quellen untersucht. Umfangreiche Ergebnisse kommen nach `research/`.

   ### 4. Entscheidungen treffen

   Der Agent macht einen konkreten Vorschlag und nennt die Folgen. Produkt-, Verhaltens- und
   Benennungsentscheidungen trifft der Nutzer.

   Eine Entscheidung gilt erst als getroffen, wenn der Nutzer ihr zugestimmt hat.

   ### 5. Entscheidungen festhalten

   Nach der Zustimmung werden zwei Stellen aktualisiert:

   - `goal.rs` zeigt die daraus entstandene Form des Ziel-Interfaces.
   - `migration.md` beschreibt die Regel, die Änderung gegenüber dem Ist-Stand und die spätere
     Prüfung.

   Kommentare in `goal.rs` bleiben kurz. Begründungen, Auswirkungen und Migrationsschritte gehören
   in `migration.md`.

   Schwerwiegende oder projektweite Entscheidungen erhalten zusätzlich eine ADR. Das gilt besonders,
   wenn eine bestehende ADR ersetzt oder eingeschränkt wird.

   ### 6. Einen Bereich vorläufig abschließen

   Ein Bereich ist für den aktuellen Planungsstand ausreichend beschrieben, wenn Folgendes klar ist:

   - seine Hauptaufgabe,
   - seine zentralen Commands und Datentypen,
   - die Bedeutung erfolgreicher Outputs,
   - die wichtigsten von außen sichtbaren Regeln,
   - bekannte offene Fragen,
   - die Auswirkungen auf den Ist-Stand.

   Danach wechseln wir zum nächsten Bereich. Details werden ergänzt, sobald sie für eine spätere
   Entscheidung oder die Implementierung notwendig werden.

   ## Regeln

   - Ziel, Ist-Stand und Migration bleiben getrennt.
   - Der aktuelle Code beschreibt den Ist-Stand, nicht das gewünschte Verhalten. Er wird verwendet,
     um Migrationsaufwand, technische Abhängigkeiten und bestehende Nutzer zu erkennen. Aus seinem
     Verhalten entstehen keine Zielanforderungen, solange diese nicht ausdrücklich beschlossen
     wurden.
   - Neue Ideen werden zuerst als gewünschtes Verhalten betrachtet. Eine Abweichung vom aktuellen
     Code ist ein Migrationshinweis und kein Gegenargument.
   - Der Agent darf bestehende Implementierungsdetails nicht stillschweigend zu fachlichen Regeln
     des Ziel-Interfaces machen.
   - Das Ziel-Interface garantiert nur das beschlossene Verhalten. Ob ein Aufrufer eine sinnvolle
     Command-Folge oder einen passenden Ausgangszustand gewählt hat, bleibt seine Verantwortung,
     sofern keine ausdrückliche Validierungsregel beschlossen wurde.
   - Ein öffentlicher Command ist nicht automatisch ein Wire-Command. Für jeden Command wird
     festgelegt, welches Modul ihn ausführt und ob er einen Transport überschreitet.
   - `goal.rs` muss nicht kompilieren.
   - Wir planen von außen nach innen und beginnen beim Verhalten für den Aufrufer.
   - Ein fachlicher Typ und eine fachliche Regel haben jeweils einen Owner.
   - Gemeinsame Abstraktionen entstehen erst, wenn mehrere konkrete Nutzer dieselbe Regel benötigen.
   - Bestehende Nutzeränderungen werden vor jeder Bearbeitung erneut gelesen und erhalten.
   - Wire-Änderungen werden ausdrücklich benannt und nicht nebenbei eingeführt.
   - Offene Details blockieren den nächsten Bereich nur, wenn sie dessen Interface verändern.
   - Sub-Agents dürfen in der aktuellen phase nicht verwendet werden, außer eine Anfrage von dir wurde vom User bestätigt.
