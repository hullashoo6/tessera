import { Component, type ReactNode } from 'react'

/**
 * If a screen hits an unexpected error, show a way back instead of a blank
 * window. Scans live in the backend, so reloading the interface keeps them.
 */
export class ErrorBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  override state = { failed: false }

  static getDerivedStateFromError() {
    return { failed: true }
  }

  override render() {
    if (!this.state.failed) return this.props.children
    return (
      <div className="empty" role="alert">
        <div>
          <h2>Something went wrong on this screen</h2>
          <p>Your files were not changed. Reloading Tessera keeps your current scan.</p>
          <button className="btn btn-primary" onClick={() => window.location.reload()}>
            Reload
          </button>
        </div>
      </div>
    )
  }
}
