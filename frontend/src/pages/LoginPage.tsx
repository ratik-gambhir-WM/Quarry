import { useRef, useState, useEffect, type FormEvent } from "react";
import { Link, Navigate, useLocation, useNavigate } from "react-router-dom";
import { useUser } from "../auth/UserProvider";
import { AppShell } from "../components/layout/AppShell";
import { BrandLockup } from "../components/brand/BrandLockup";
import { Icon } from "../components/ui/Icon";
import { FormField } from "../components/ui/FormField";
import { QuarryButton } from "../components/ui/QuarryButton";
export function LoginPage({ register = false }: { register?: boolean }) {
  const user = useUser();
  const navigate = useNavigate();
  const location = useLocation();
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  const [visible, setVisible] = useState(false);
  const active = useRef(true);
  useEffect(() => { active.current = true; return () => { active.current = false; }; }, []);
  const from: unknown = location.state && typeof location.state === "object" && "from" in location.state ? location.state.from : undefined;
  const destination = typeof from === "string" && from.startsWith("/hub") && !from.includes("\\") ? from : "/hub";
  if (user.user) return <Navigate replace to={destination} />;
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    const values = new FormData(event.currentTarget);
    const value = (name: string) => {
      const entry = values.get(name);
      return typeof entry === "string" ? entry : "";
    };
    if (register && value("password") !== value("confirm")) { setError("Passwords do not match."); return; }
    setError(""); setPending(true);
    try {
      const input = { username: value("username").trim(), password: value("password") };
      const jobTitle = value("jobTitle").trim();
      if (register) await user.register({ ...input, email: `${value("email").trim().replace(/@.*$/, "")}@westmonroe.com`, firstName: value("firstName").trim(), lastName: value("lastName").trim(), ...(jobTitle ? { jobTitle } : {}) }, () => active.current);
      else await user.login(input, () => active.current);
      if (active.current) navigate(destination, { replace: true });
    } catch (failure) { if (active.current) setError(failure instanceof Error ? failure.message : "Unable to sign in. Try again."); }
    finally { if (active.current) setPending(false); }
  }
  return (
    <AppShell centered showFooter={false}>
      <section className={`glass-panel mx-auto w-full rounded-[8px] p-lg sm:p-xl ${register ? "max-w-[900px]" : "max-w-[560px]"}`}>
        <BrandLockup compact={register} title="Quarry" subtitle={register ? "Create your account" : "Sign in to your account"} />
        <form className="mt-lg space-y-md" onSubmit={submit} aria-busy={pending}>
          <div className={`grid gap-md ${register ? "sm:grid-cols-2" : ""}`}>
            <FormField className="py-sm" icon={<Icon name="personSearch" className="h-5 w-5" />} id="username" name="username" label="Username" autoComplete="username" minLength={3} maxLength={32} />
            {register && (
              <>
                <div className="min-w-0 space-y-xs">
                  <label className="type-label px-xs text-on-surface-variant" htmlFor="email">West Monroe email</label>
                  <div className="flex overflow-hidden rounded-full border border-outline-variant bg-surface-container-lowest focus-within:border-primary-container focus-within:ring-4 focus-within:ring-primary-fixed/40">
                    <input
                      className="min-w-0 flex-1 bg-transparent px-md py-sm text-[16px] leading-[1.6] text-on-surface outline-none"
                      id="email"
                      name="email"
                      aria-describedby="email-domain"
                      autoComplete="email"
                      maxLength={300}
                      placeholder="your.name"
                      required
                      type="text"
                      onChange={(event) => {
                        const name = event.currentTarget.value.replace(/\s/g, "").replace(/@.*$/, "");
                        event.currentTarget.value = name;
                      }}
                    />
                    <span className="flex shrink-0 items-center border-l border-outline-variant bg-surface-container-low px-sm text-sm font-semibold text-on-surface-variant" id="email-domain">@westmonroe.com</span>
                  </div>
                </div>
                <FormField className="py-sm" icon={<Icon name="personSearch" className="h-5 w-5" />} id="first-name" name="firstName" label="First name" autoComplete="given-name" maxLength={100} />
                <FormField className="py-sm" icon={<Icon name="personSearch" className="h-5 w-5" />} id="last-name" name="lastName" label="Last name" autoComplete="family-name" maxLength={100} />
              </>
            )}
            <FormField className="py-sm" icon={<Icon name="key" className="h-5 w-5" />} id="password" name="password" label="Password" type={visible ? "text" : "password"} autoComplete={register ? "new-password" : "current-password"} />
            {register && (
              <>
                <FormField className="py-sm" icon={<Icon name="key" className="h-5 w-5" />} id="confirm-password" name="confirm" label="Confirm password" type={visible ? "text" : "password"} autoComplete="new-password" />
                <div className="sm:col-span-2">
                  <FormField className="py-sm" icon={<Icon name="personSearch" className="h-5 w-5" />} required={false} id="job-title" name="jobTitle" label="Job title (optional)" autoComplete="organization-title" maxLength={200} />
                </div>
              </>
            )}
          </div>
          <div className="flex flex-wrap items-start justify-between gap-sm">
            <button type="button" className="shrink-0 text-sm text-primary underline focus-visible:outline-2" aria-pressed={visible} onClick={() => setVisible(!visible)}>{visible ? "Hide password" : "Show password"}</button>
            {register && <p className="type-body-sm max-w-[32rem] text-muted">Use 15–128 characters. Usernames use 3–32 letters, numbers, dots, underscores or hyphens.</p>}
          </div>
          {error && <p className="text-error" role="alert">{error}</p>}
          {user.notice && <p className="text-secondary" role="status">{user.notice}</p>}
          <QuarryButton type="submit" disabled={pending}>{pending ? "Please wait…" : register ? "Create account" : "Sign in"}</QuarryButton>
          <span className="sr-only" role="status">{pending ? register ? "Creating account…" : "Signing in…" : ""}</span>
        </form>
        <p className="mt-md type-body-sm">{register ? "Already have an account? " : "New to Quarry? "}<Link className="text-primary underline" to={register ? "/login" : "/register"} state={{ from: destination }}>{register ? "Sign in" : "Create an account"}</Link></p>
      </section>
    </AppShell>
  );
}
