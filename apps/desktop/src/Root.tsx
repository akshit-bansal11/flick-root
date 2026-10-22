import { useCallback, useEffect, useRef, useState } from "react";
import * as ipc from "./ipc";
import { Launcher } from "./launcher/Launcher";
import type { MenuAction } from "./tray/menuReducer";
import { TrayMenu } from "./tray/TrayMenu";

const warn = (e: unknown) => console.error(e);

/** The prewarmed window's one root: the launcher, or the tray menu between a right-click and the next hide or show. */
export function Root() {
  const [menu, setMenu] = useState<ipc.TrayMenuEvent | null>(null);
  const [shown, setShown] = useState<ipc.ShownEvent | null>(null);
  const open = useRef(false);
  open.current = menu !== null;

  useEffect(() => {
    const unlisten = [
      ipc.onTrayMenu(setMenu),
      ipc.onHidden(() => {
        setShown(null);
        setMenu(null);
      }),
      ipc.onShown((p) => {
        // Only a show that ends the menu needs replaying: the launcher is unmounted and missed it.
        setShown(open.current ? p : null);
        setMenu(null);
      }),
    ];
    return () => {
      for (const u of unlisten) u.then((fn) => fn()).catch(warn);
    };
  }, []);

  const onRun = useCallback((action: MenuAction) => {
    ipc.trayAction(action).catch(warn);
  }, []);
  const onClose = useCallback(() => {
    ipc.hideLauncher().catch(warn);
  }, []);

  return menu ? (
    <TrayMenu event={menu} onRun={onRun} onClose={onClose} />
  ) : (
    <Launcher initialShown={shown} />
  );
}
