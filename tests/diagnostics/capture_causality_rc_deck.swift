import AppKit

final class KeyDeckWindow: NSWindow {
    override var canBecomeKey: Bool { true }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)

let screens = NSScreen.screens.map(\.frame)
guard var desktop = screens.first else {
    fputs("no NSScreen\n", stderr)
    exit(2)
}
for frame in screens.dropFirst() {
    desktop = desktop.union(frame)
}
let desktopFrame = desktop

let tiny = NSRect(x: desktopFrame.maxX - 6, y: desktopFrame.maxY - 6, width: 4, height: 4)
let deck = KeyDeckWindow(
    contentRect: tiny,
    styleMask: [.borderless],
    backing: .buffered,
    defer: false
)
deck.backgroundColor = .black
deck.isOpaque = true
deck.ignoresMouseEvents = true
deck.level = .floating

@MainActor
func acknowledge(_ state: String) {
    let frame = deck.frame
    print("{\"deck_state\":\"\(state)\",\"key\":\(deck.isKeyWindow)," +
          "\"x\":\(frame.origin.x),\"y\":\(frame.origin.y)," +
          "\"width\":\(frame.size.width),\"height\":\(frame.size.height)," +
          "\"desktop_x\":\(desktopFrame.origin.x),\"desktop_y\":\(desktopFrame.origin.y)," +
          "\"desktop_width\":\(desktopFrame.size.width),\"desktop_height\":\(desktopFrame.size.height)}")
    fflush(stdout)
}

@MainActor
func show(_ frame: NSRect, state: String) {
    deck.setFrame(frame, display: true)
    deck.makeKeyAndOrderFront(nil)
    app.activate(ignoringOtherApps: true)
    acknowledge(state)
}

DispatchQueue.global(qos: .userInitiated).async {
    while let command = readLine() {
        let words = command.split(separator: " ").map(String.init)
        DispatchQueue.main.sync {
            MainActor.assumeIsolated {
            switch words.first {
            case "small" where words.count == 1:
                show(tiny, state: "small")
            case "partial" where words.count == 5, "cover" where words.count == 5:
                guard let x = Double(words[1]), let y = Double(words[2]),
                      let width = Double(words[3]), let height = Double(words[4]) else {
                    acknowledge("invalid")
                    return
                }
                if words[0] == "partial" {
                    show(
                        NSRect(
                            x: x + width / 2.0,
                            y: y - 8.0,
                            width: width / 2.0 + 8.0,
                            height: height + 16.0
                        ),
                        state: "partial"
                    )
                } else {
                    show(
                        NSRect(x: x - 8.0, y: y - 8.0, width: width + 16.0, height: height + 16.0),
                        state: "cover"
                    )
                }
            case "quit" where words.count == 1:
                deck.orderOut(nil)
                acknowledge("quit")
                app.terminate(nil)
            default:
                acknowledge("unknown")
            }
            }
        }
    }
}

app.run()
