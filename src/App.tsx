import { lazy, Suspense, useCallback, useEffect, useState } from "react";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { ipc } from "./lib/ipc";
import { useHealth } from "./lib/useHealth";
import { StaffDot } from "./components/StaffDot";
import { useKiosk } from "./kiosk/useKiosk";
import { NameScreen } from "./kiosk/screens/NameScreen";
import { SearchScreen } from "./kiosk/screens/SearchScreen";
import { ChooseScreen } from "./kiosk/screens/ChooseScreen";
import { DoneScreen } from "./kiosk/screens/DoneScreen";

const PinPad = lazy(() => import("./admin/PinPad"));
const AdminOverlay = lazy(() => import("./admin/AdminOverlay"));

type AdminStage = "closed" | "pin" | "open";

export default function App() {
  const { state, config, dispatch, pickTrack, pickManual, commit, cancelChoose } = useKiosk();
  const health = useHealth();
  const [adminStage, setAdminStage] = useState<AdminStage>("closed");
  const reduced = useReducedMotion();
  const adminOpen = adminStage !== "closed";

  const closeAdmin = useCallback(() => {
    void ipc.adminLock().catch(() => undefined);
    setAdminStage("closed");
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.altKey && event.shiftKey && event.code === "KeyK") {
        event.preventDefault();
        setAdminStage((stage) => (stage === "closed" ? "pin" : stage));
        return;
      }
      if ((event.metaKey || event.ctrlKey) && ["+", "-", "=", "0"].includes(event.key)) {
        event.preventDefault();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const unlock = async (pin: string) => {
    await ipc.adminUnlock(pin);
    setAdminStage("open");
  };

  const renderScreen = () => {
    switch (state.screen) {
      case "name":
        return (
          <NameScreen
            welcome={config?.welcome ?? "A song for the baby"}
            name={state.name}
            onChange={(name) => dispatch({ type: "SET_NAME", name })}
            onSubmit={() => dispatch({ type: "NAME_SUBMIT" })}
          />
        );
      case "search":
        return (
          <SearchScreen
            guestName={state.guestName}
            babyName={config?.babyName ?? "the baby"}
            query={state.query}
            results={state.results}
            degraded={state.degraded}
            highlighted={state.highlighted}
            onQueryChange={(query) => dispatch({ type: "SET_QUERY", query })}
            onMove={(delta) => dispatch({ type: "MOVE_HIGHLIGHT", delta })}
            onHighlight={(index) => dispatch({ type: "HIGHLIGHT", index })}
            onPick={pickTrack}
            onManual={pickManual}
            onBack={() => dispatch({ type: "BACK" })}
          />
        );
      case "choose":
        return (
          <ChooseScreen
            active={!adminOpen}
            selection={state.selection}
            prep={state.prep}
            orientation={state.orientation}
            layout={state.layout}
            onOrientation={(orientation) => dispatch({ type: "SET_ORIENTATION", orientation })}
            onLayout={(layout) => dispatch({ type: "SET_LAYOUT", layout })}
            onCommit={(orientation, layout) => void commit(orientation, layout)}
            onBack={cancelChoose}
          />
        );
      case "done":
        return (
          <DoneScreen
            active={!adminOpen}
            guestName={state.guestName}
            orientation={state.orientation}
            printFile={state.printFile}
            printsAhead={state.printsAhead}
            onNext={() => dispatch({ type: "RESET" })}
          />
        );
    }
  };

  return (
    <div className="relative h-full w-full overflow-hidden">
      <StaffDot attention={Boolean(health?.attention)} />
      <div className="kiosk-stage h-full w-full">
        <AnimatePresence mode="wait">
          <motion.main
            key={state.screen}
            initial={{ opacity: 0, y: reduced ? 0 : 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: reduced ? 0 : -10 }}
            transition={{ duration: reduced ? 0 : 0.2, ease: "easeOut" }}
            className="h-full w-full overflow-y-auto overscroll-contain px-12"
          >
            <div className="flex min-h-full w-full flex-col">
              <div className="my-auto flex w-full justify-center">{renderScreen()}</div>
            </div>
          </motion.main>
        </AnimatePresence>
      </div>

      {adminStage === "pin" ? (
        <Suspense fallback={null}>
          <PinPad onUnlock={unlock} onClose={() => setAdminStage("closed")} />
        </Suspense>
      ) : null}

      {adminStage === "open" ? (
        <Suspense fallback={null}>
          <AdminOverlay onClose={closeAdmin} />
        </Suspense>
      ) : null}
    </div>
  );
}
