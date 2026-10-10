import { ErrorBoundary } from "./components/ErrorBoundary";
import { Toaster } from "./components/Toaster";
import { SectionView } from "./shell/SectionView";
import { Sidebar } from "./shell/Sidebar";
import { useUiStore } from "./state/ui";

export function App() {
  const section = useUiStore((state) => state.section);

  return (
    <ErrorBoundary>
      <div className="shell">
        <Sidebar />
        <main className="shell__content">
          <SectionView section={section} />
        </main>
      </div>
      <Toaster />
    </ErrorBoundary>
  );
}
