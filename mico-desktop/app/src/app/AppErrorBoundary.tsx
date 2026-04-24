import { Component, type ReactNode } from "react";

interface AppErrorBoundaryProps {
  children: ReactNode;
}

interface AppErrorBoundaryState {
  error: Error | null;
}

export class AppErrorBoundary extends Component<AppErrorBoundaryProps, AppErrorBoundaryState> {
  constructor(props: AppErrorBoundaryProps) {
    super(props);
    this.state = { error: null };
  }

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error) {
    console.error("mico renderer crashed", error);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="crash-screen" role="alert">
          <div className="crash-screen__panel">
            <h1>mico hit a runtime error</h1>
            <p>{this.state.error.message || "The renderer crashed while handling the last action."}</p>
            <div className="crash-screen__actions">
              <button onClick={() => window.location.reload()} type="button">
                Reload app
              </button>
            </div>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
