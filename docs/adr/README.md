# Entscheidungsherkunft

Die nummerierten ADRs bewahren Entscheidungen samt damaligem Kontext und verworfenen
Alternativen. Ihr Status bezeichnet die jeweilige historische Entscheidung.
Sie sind keine parallel fortzuschreibende Spezifikation des Rust-Rewrites.

Für dessen gültige Regeln gilt ausschließlich [target.md](../api/target.md).
Die Konsolidierung übernimmt insbesondere die fachlichen Verträge aus ADR-0004 bis ADR-0019
und die Verantwortlichkeiten aus ADR-0023/0024. Die älteren Zugangsmodelle aus
ADR-0020 bis ADR-0022 sind als historische Entwürfe zu lesen.

Neue Erkenntnisse ändern den Zielvertrag an seiner maßgeblichen Stelle.
Eine neue ADR ist nur bei einer schwer umkehrbaren Entscheidung mit erklärungsbedürftigem
Abwägen sinnvoll; sie ersetzt keine laufende Pflege des Zielvertrags.

[ADR-0025](0025-headless-agent-interaction.md) legt Headless-Interaktion als
Produktziel fest. Die vorhandenen Timing-, Session- und Replay-Entscheidungen
bleiben erhalten. Seine ursprüngliche offene/enge Integrationsreichweite ist
historischer Kontext; der aktuelle transparente Window-/Kamera-Vertrag und die
Abgrenzung der Imagefixtures stehen in
[headless-integration.md](../api/headless-integration.md). Die
Headless-Implementation ist damit noch nicht abgeschlossen.
