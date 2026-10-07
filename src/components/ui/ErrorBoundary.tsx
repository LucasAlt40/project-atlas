import { Component, type ErrorInfo, type ReactNode } from 'react';
import { useT } from '@/i18n/I18nProvider';
import { Button } from './Button';

interface BoundaryProps {
  children: ReactNode;
  /** When it changes the boundary tries again (for example, the person went to another screen). */
  resetKey?: string | undefined;
  title: string;
  retryLabel: string;
}

interface BoundaryState {
  failed: boolean;
}

class Boundary extends Component<BoundaryProps, BoundaryState> {
  override state: BoundaryState = { failed: false };

  static getDerivedStateFromError(): BoundaryState {
    return { failed: true };
  }

  override componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('A screen failed to render', error, info.componentStack);
  }

  override componentDidUpdate(previous: BoundaryProps) {
    if (this.state.failed && previous.resetKey !== this.props.resetKey) {
      this.setState({ failed: false });
    }
  }

  override render() {
    if (!this.state.failed) return this.props.children;
    return (
      <div role="alert" style={{ padding: 24, display: 'grid', gap: 12, justifyItems: 'start' }}>
        <strong>{this.props.title}</strong>
        <Button
          variant="secondary"
          onClick={() => {
            this.setState({ failed: false });
          }}
        >
          {this.props.retryLabel}
        </Button>
      </div>
    );
  }
}

/**
 * Keeps a rendering error in one part of the app from blanking the whole window: the rest of the
 * app stays usable and the person can try again.
 */
export function ErrorBoundary({ children, resetKey }: { children: ReactNode; resetKey?: string }) {
  const t = useT();
  return (
    <Boundary resetKey={resetKey} title={t('common.renderFailed')} retryLabel={t('common.retry')}>
      {children}
    </Boundary>
  );
}
