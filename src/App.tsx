import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { api } from "./api";
import Dictionaries from "./pages/Dictionaries";
import Library from "./pages/Library";
import Reader from "./pages/Reader";

type Screen = { name: "library" } | { name: "reader"; bookId: string } | { name: "dictionaries" };

export default function App() {
  const [screen, setScreen] = useState<Screen>({ name: "library" });
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    api.getSettings().then(async (settings) => {
      if (settings.windowWidth > 400 && settings.windowHeight > 300) {
        await getCurrentWindow().setSize(new LogicalSize(settings.windowWidth, settings.windowHeight));
      }
    }).catch(() => undefined);

    const win = getCurrentWindow();
    let timer = 0;
    const unlistenPromise = win.onResized(() => {
      window.clearTimeout(timer);
      timer = window.setTimeout(async () => {
        const size = await win.innerSize();
        const scale = await win.scaleFactor();
        const logical = size.toLogical(scale);
        const settings = await api.getSettings();
        await api.saveSettings({ ...settings, windowWidth: logical.width, windowHeight: logical.height });
      }, 400);
    });

    async function openPaths(paths: string[]) {
      const files = paths.filter(Boolean);
      if (!files.length) return;
      const report = await api.importPaths(files);
      setRefreshKey((value) => value + 1);
      const book = report.added[0] ?? await api.bookByPath(files[0]);
      if (book) setScreen({ name: "reader", bookId: book.id });
    }

    api.takeLaunchFiles().then(openPaths).catch(() => undefined);
    const filesPromise = listen<string[]>("open-files", (event) => {
      openPaths(event.payload).catch(() => undefined);
    });

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
      filesPromise.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <div className="app">
      {screen.name !== "reader" && (
        <header className="topbar">
          <button className="brand" onClick={() => setScreen({ name: "library" })}>Book Reader</button>
          <nav className="nav">
            <button className={screen.name === "library" ? "active" : ""} onClick={() => setScreen({ name: "library" })}>Library</button>
            <button className={screen.name === "dictionaries" ? "active" : ""} onClick={() => setScreen({ name: "dictionaries" })}>Dictionaries</button>
          </nav>
        </header>
      )}
      {screen.name === "library" && (
        <Library refreshKey={refreshKey} onOpen={(bookId) => setScreen({ name: "reader", bookId })} />
      )}
      {screen.name === "dictionaries" && <Dictionaries />}
      {screen.name === "reader" && (
        <Reader
          bookId={screen.bookId}
          onBack={() => setScreen({ name: "library" })}
          onDictionaries={() => setScreen({ name: "dictionaries" })}
        />
      )}
    </div>
  );
}
