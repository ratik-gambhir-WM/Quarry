import { Input } from "../components/ui/input";
import { useRef, useState, useEffect, type FormEvent } from "react";
import { motion, useReducedMotion } from "motion/react";
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
  const [step, setStep] = useState<"username" | "password">("username");
  const [identity, setIdentity] = useState("");
  const formRef = useRef<HTMLFormElement>(null);
  const reduceMotion = useReducedMotion();
  const hasChangedStep = useRef(false);
  useEffect(() => {
    if (!hasChangedStep.current) return;
    const field = formRef.current?.elements.namedItem(step);
    if (field instanceof HTMLInputElement) field.focus();
  }, [step]);
  function continueToPassword() {
    const field = formRef.current?.elements.namedItem("username");
    if (!(field instanceof HTMLInputElement) || !field.reportValidity()) return;
    setIdentity(field.value.trim());
    setError("");
    hasChangedStep.current = true;
    setStep("password");
  }
  function backToUsername() {
    setError("");
    hasChangedStep.current = true;
    setStep("username");
  }
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  const from: unknown =
    location.state &&
    typeof location.state === "object" &&
    "from" in location.state
      ? location.state.from
      : undefined;
  const destination =
    typeof from === "string" && from.startsWith("/hub") && !from.includes("\\")
      ? from
      : "/hub";
  if (user.user) return <Navigate replace to={destination} />;
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    const values = new FormData(event.currentTarget);
    const value = (name: string) => {
      const entry = values.get(name);
      return typeof entry === "string" ? entry : "";
    };
    if (register && value("password") !== value("confirm")) {
      setError("Passwords do not match.");
      return;
    }
    setError("");
    setPending(true);
    try {
      const input = {
        username: value("username").trim(),
        password: value("password"),
      };
      const jobTitle = value("jobTitle").trim();
      if (register)
        await user.register(
          {
            ...input,
            email: `${value("email").trim().replace(/@.*$/, "")}@westmonroe.com`,
            firstName: value("firstName").trim(),
            lastName: value("lastName").trim(),
            ...(jobTitle ? { jobTitle } : {}),
          },
          () => active.current,
        );
      else await user.login(input, () => active.current);
      if (active.current) navigate(destination, { replace: true });
    } catch (failure) {
      if (active.current)
        setError(
          failure instanceof Error
            ? failure.message
            : "Unable to sign in. Try again.",
        );
    } finally {
      if (active.current) setPending(false);
    }
  }
  if (!register) {
    return (
      <AppShell centered showFooter={false}>
        <motion.section
          layout={!reduceMotion}
          transition={{ duration: reduceMotion ? 0 : 0.34, ease: "easeInOut" }}
          className="glass-panel mx-auto w-full max-w-[400px] overflow-hidden rounded-xl p-lg sm:p-xl"
        >
          <BrandLockup title="Quarry" subtitle="Sign in to your account" />
          <form ref={formRef} className="mt-lg" onSubmit={submit} aria-busy={pending}>
            <motion.div layout={!reduceMotion}>
              <div className="auth-step auth-step-back" hidden={step !== "username"}>
                <div className="space-y-md">
                  <FormField
                    className="py-sm"
                    icon={<Icon name="personSearch" className="h-5 w-5" />}
                    id="username" name="username" label="Username"
                    autoComplete="username" minLength={3} maxLength={32}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        event.preventDefault();
                        continueToPassword();
                      }
                    }}
                  />
                  {user.notice && <p className="text-secondary" role="status">{user.notice}</p>}
                  <QuarryButton type="button" onClick={continueToPassword}>Continue</QuarryButton>
                  <p className="text-center type-body-sm">
                    New to Quarry?{" "}
                    <Link className="text-primary underline" to="/register" state={{ from: destination }}>
                      Create an account
                    </Link>
                  </p>
                </div>
              </div>
              <div className="auth-step auth-step-forward" hidden={step !== "password"}>
                <div className="space-y-md">
                  <div>
                    <h2 className="type-label text-on-surface">Enter your password</h2>
                    <span className="mt-sm inline-flex max-w-full items-center gap-sm rounded-full border border-outline-variant bg-surface-container-low px-sm py-xs text-sm text-on-surface">
                      <span aria-hidden="true" className="grid h-6 w-6 shrink-0 place-items-center rounded-full bg-primary-fixed text-primary">{identity.charAt(0).toUpperCase()}</span>
                      <span className="truncate">{identity}</span>
                    </span>
                  </div>
                  <FormField
                    className="py-sm"
                    id="password" name="password" label="Password"
                    type="password" autoComplete="current-password"
                  />
                  {error && <p className="text-error" role="alert">{error}</p>}
                  {user.notice && <p className="text-secondary" role="status">{user.notice}</p>}
                  <QuarryButton type="submit" disabled={pending}>
                    {pending ? "Please wait…" : "Sign in"}
                  </QuarryButton>
                  <span className="sr-only" role="status">{pending ? "Signing in…" : ""}</span>
                  <button type="button" disabled={pending} onClick={backToUsername} className="text-sm text-primary underline focus-visible:outline-2 disabled:opacity-60">
                    ← Use a different username
                  </button>
                </div>
              </div>
            </motion.div>
          </form>
        </motion.section>
      </AppShell>
    );
  }
  return (
    <AppShell centered showFooter={false}>
      <section
        className={`glass-panel mx-auto w-full rounded-[8px] p-lg sm:p-xl ${register ? "max-w-[900px]" : "max-w-[560px]"}`}
      >
        <BrandLockup
          compact={register}
          title="Quarry"
          subtitle={
            register ? "Create your account" : "Sign in to your account"
          }
        />
        <form
          className="mt-lg space-y-md"
          onSubmit={submit}
          aria-busy={pending}
        >
          <div className={`grid gap-md ${register ? "sm:grid-cols-2" : ""}`}>
            <FormField
              className="py-sm"
              icon={<Icon name="personSearch" className="h-5 w-5" />}
              id="username"
              name="username"
              label="Username"
              autoComplete="username"
              minLength={3}
              maxLength={32}
            />
            {register && (
              <>
                <div className="min-w-0 space-y-xs">
                  <label
                    className="type-label px-xs text-on-surface-variant"
                    htmlFor="email"
                  >
                    West Monroe email
                  </label>
                  <div className="flex overflow-hidden rounded-full border border-outline-variant bg-surface-container-lowest focus-within:border-primary-container focus-within:ring-4 focus-within:ring-primary-fixed/40">
                    <Input
                      className="h-auto min-w-0 flex-1 rounded-none border-0 bg-transparent px-md py-sm text-[16px] leading-[1.6] text-on-surface shadow-none outline-none focus-visible:ring-0"
                      id="email"
                      name="email"
                      aria-describedby="email-domain"
                      autoComplete="email"
                      maxLength={300}
                      placeholder="your.name"
                      required
                      type="text"
                      onChange={(event) => {
                        const name = event.currentTarget.value
                          .replace(/\s/g, "")
                          .replace(/@.*$/, "");
                        event.currentTarget.value = name;
                      }}
                    />
                    <span
                      className="flex shrink-0 items-center border-l border-outline-variant bg-surface-container-low px-sm text-sm font-semibold text-on-surface-variant"
                      id="email-domain"
                    >
                      @westmonroe.com
                    </span>
                  </div>
                </div>
                <FormField
                  className="py-sm"
                  icon={<Icon name="personSearch" className="h-5 w-5" />}
                  id="first-name"
                  name="firstName"
                  label="First name"
                  autoComplete="given-name"
                  maxLength={100}
                />
                <FormField
                  className="py-sm"
                  icon={<Icon name="personSearch" className="h-5 w-5" />}
                  id="last-name"
                  name="lastName"
                  label="Last name"
                  autoComplete="family-name"
                  maxLength={100}
                />
              </>
            )}
            <FormField
              className="py-sm"
              id="password"
              name="password"
              label="Password"
              type="password"
              autoComplete={register ? "new-password" : "current-password"}
            />
            {register && (
              <>
                <FormField
                  className="py-sm"
                      id="confirm-password"
                  name="confirm"
                  label="Confirm password"
                  type="password"
                  autoComplete="new-password"
                />
                <div className="sm:col-span-2">
                  <FormField
                    className="py-sm"
                    icon={<Icon name="personSearch" className="h-5 w-5" />}
                    required={false}
                    id="job-title"
                    name="jobTitle"
                    label="Job title (optional)"
                    autoComplete="organization-title"
                    maxLength={200}
                  />
                </div>
              </>
            )}
          </div>
          {register && (
            <p className="type-body-sm max-w-[32rem] text-muted">
              Use 15–128 characters. Usernames use 3–32 letters, numbers,
              dots, underscores or hyphens.
            </p>
          )}
          {error && (
            <p className="text-error" role="alert">
              {error}
            </p>
          )}
          {user.notice && (
            <p className="text-secondary" role="status">
              {user.notice}
            </p>
          )}
          <QuarryButton type="submit" disabled={pending}>
            {pending ? "Please wait…" : register ? "Create account" : "Sign in"}
          </QuarryButton>
          <span className="sr-only" role="status">
            {pending ? (register ? "Creating account…" : "Signing in…") : ""}
          </span>
        </form>
        <p className="mt-md type-body-sm">
          {register ? "Already have an account? " : "New to Quarry? "}
          <Link
            className="text-primary underline"
            to={register ? "/login" : "/register"}
            state={{ from: destination }}
          >
            {register ? "Sign in" : "Create an account"}
          </Link>
        </p>
      </section>
    </AppShell>
  );
}
