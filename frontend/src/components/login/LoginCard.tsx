import { FormEvent } from "react";
import { BrandLockup } from "../brand/BrandLockup";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";

const WEST_MONROE_EMAIL_DOMAIN = "@westmonroe.com";

type LoginCardProps = {
  email: string;
  error?: string;
  isChecking?: boolean;
  onEmailChange: (value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  statusText?: string;
};

export function LoginCard({
  email,
  error = "",
  isChecking = false,
  onEmailChange,
  onSubmit,
  statusText = "",
}: LoginCardProps) {
  const emailLocalPart = getEmailLocalPart(email);

  function handleEmailLocalPartChange(value: string) {
    const localPart = value.replace(/\s/g, "").replace(/@.*$/, "");
    onEmailChange(localPart ? `${localPart}${WEST_MONROE_EMAIL_DOMAIN}` : "");
  }

  return (
    <section className="glass-panel mx-auto w-full max-w-[440px] rounded-[8px] p-xl">
      <BrandLockup
        subtitle="Precision insights for executive decision-makers."
        title="Strategic Portfolio Hub"
      />

      <form className="mt-xl space-y-lg" onSubmit={onSubmit}>
        <div className="space-y-xs">
          <div className="flex items-center justify-between gap-sm px-xs">
            <label className="type-label text-on-surface-variant" htmlFor="email-local-part">
              WM Email
            </label>
          </div>

          <div className="flex overflow-hidden rounded-full border border-outline-variant bg-surface-container-lowest shadow-[inset_0_1px_0_rgba(255,255,255,0.5)] transition focus-within:border-primary-container focus-within:ring-4 focus-within:ring-primary-fixed/40">
            <input
              autoComplete="username"
              className="min-w-0 flex-1 bg-transparent py-md pl-md pr-sm text-[16px] leading-[1.6] text-on-surface outline-none placeholder:text-outline-variant"
              id="email-local-part"
              onChange={(event) => handleEmailLocalPartChange(event.currentTarget.value)}
              placeholder="rgambhir"
              required
              type="text"
              value={emailLocalPart}
            />
            <div className="flex shrink-0 items-center border-l border-outline-variant bg-surface-container-low px-md text-[14px] font-semibold tracking-[0.02em] text-muted">
              {WEST_MONROE_EMAIL_DOMAIN}
            </div>
          </div>
        </div>

        {statusText || error ? (
          <p className={`px-xs text-[14px] font-medium ${error ? "text-error" : "text-muted"}`}>
            {error || statusText}
          </p>
        ) : null}

        <Button disabled={isChecking} icon={<Icon className="h-[18px] w-[18px]" name="arrowRight" />} type="submit">
          {isChecking ? "Checking..." : "Continue"}
        </Button>
      </form>

      <div className="mt-xl flex items-start gap-md border-t border-outline-variant/80 pt-lg">
        <div className="mt-0.5 text-outline">
          <Icon className="h-5 w-5" name="shield" />
        </div>
        <p className="type-body-sm leading-relaxed text-secondary">
          <span className="font-semibold text-on-surface">Privacy &amp; Security:</span> We'll check whether your
          workspace profile exists before asking for any additional setup details.
        </p>
      </div>
    </section>
  );
}

function getEmailLocalPart(email: string) {
  return email.split("@")[0] ?? "";
}
