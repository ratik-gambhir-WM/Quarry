import { Link } from "react-router-dom";
import { AppShell } from "../components/layout/AppShell";
import { buttonVariants } from "../components/ui/button";

export function MissingPageContent({ workspace = false }: { workspace?: boolean }) {
  return (
    <section className="mx-auto flex w-full max-w-[28rem] flex-col items-center gap-4 py-16 text-center">
      <p className="type-label text-muted">404 · Page not found</p>
      <h2 className="type-h1 text-on-surface">This page isn’t here</h2>
      <p className="type-body-sm text-on-surface-variant">
        The link may be out of date, or the page may have moved.
      </p>
      <Link className={buttonVariants({ className: "mt-2" })} to={workspace ? "/hub" : "/login"}>
        {workspace ? "Back to home" : "Back to sign in"}
      </Link>
    </section>
  );
}

export function NotFoundPage() {
  return (
    <AppShell centered showFooter={false}>
      <h1 className="sr-only">Page not found</h1>
      <MissingPageContent />
    </AppShell>
  );
}
