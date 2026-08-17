import { Component, useState, type ErrorInfo, type PropsWithChildren } from "react";

import { routes, type RouteKey } from "./app/routes";
import { AppLayout } from "./components/layout/AppLayout";
import DashboardPage from "./pages/Dashboard";
import SettingsPage from "./pages/Settings";
import ProjectsPage from "./pages/Projects";
import ModelsPage from "./pages/Models";
import { initialAppState } from "./stores/app";
import { ErrorState } from "./components/common/ErrorState";

interface ErrorBoundaryState {
  hasError: boolean;
  message: string;
}

class AppErrorBoundary extends Component<PropsWithChildren, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false, message: "" };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, message: error.message };
  }

  componentDidCatch(error: Error, _errorInfo: ErrorInfo) {
    console.error("Application render error", error);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="app-error-boundary">
          <ErrorState
            title="The application could not render"
            message={this.state.message || "An unexpected UI error occurred."}
          />
        </div>
      );
    }

    return this.props.children;
  }
}

function CurrentPage({ route }: { route: RouteKey }) {
  switch (route) {
    case routes.settings.key:
      return <SettingsPage />;
    case routes.projects.key:
      return <ProjectsPage />;
    case routes.models.key:
      return <ModelsPage />;
    case routes.overview.key:
    default:
      return <DashboardPage />;
  }
}

function App() {
  const [activeRoute, setActiveRoute] = useState<RouteKey>(initialAppState.activeRoute);

  return (
    <AppErrorBoundary>
      <AppLayout activeRoute={activeRoute} onNavigate={setActiveRoute}>
        <CurrentPage route={activeRoute} />
      </AppLayout>
    </AppErrorBoundary>
  );
}

export default App;
