import { FormEvent, useState } from "react";
import { useNavigate } from "react-router-dom";
import { runtime } from "@quarry/runtime";
import { LoginCard } from "../components/login/LoginCard";
import { AppShell } from "../components/layout/AppShell";
import { QuarryButton } from "../components/ui/QuarryButton";
import { FormField } from "../components/ui/FormField";
import { Icon } from "../components/ui/Icon";
import { persistWorkspaceEmail } from "../hooks/useWorkspaceSession";

type LoginStep = "email" | "new-user";

type NewUserFormState = {
  apiKey: string;
  firstName: string;
  lastName: string;
  role: string;
};

const emptyNewUserForm: NewUserFormState = {
  apiKey: "",
  firstName: "",
  lastName: "",
  role: "",
};

export function LoginPage() {
  const navigate = useNavigate();
  const [email, setEmail] = useState("");
  const [error, setError] = useState("");
  const [isCheckingUser, setIsCheckingUser] = useState(false);
  const [isCreatingUser, setIsCreatingUser] = useState(false);
  const [newUserForm, setNewUserForm] = useState<NewUserFormState>(emptyNewUserForm);
  const [step, setStep] = useState<LoginStep>("email");

  async function handleEmailSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const normalizedEmail = email.trim();

    if (!normalizedEmail) {
      setError("Enter your West Monroe email.");
      return;
    }

    setError("");
    setIsCheckingUser(true);

    try {
      const exists = await runtime.api.userExistsByEmail(normalizedEmail);

      if (exists) {
        enterWorkspace(normalizedEmail);
        return;
      }

      setStep("new-user");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCheckingUser(false);
    }
  }

  async function handleCreateUser(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const normalizedEmail = email.trim();

    setError("");
    setIsCreatingUser(true);

    try {
      await runtime.api.createUser({
        apiKey: newUserForm.apiKey.trim(),
        email: normalizedEmail,
        firstName: newUserForm.firstName.trim(),
        lastName: newUserForm.lastName.trim(),
        role: newUserForm.role.trim(),
      });

      enterWorkspace(normalizedEmail);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCreatingUser(false);
    }
  }

  function enterWorkspace(workspaceEmail: string) {
    persistWorkspaceEmail(workspaceEmail);

    navigate("/hub", {
      state: {
        email: workspaceEmail,
      },
    });
  }

  function updateNewUserField(field: keyof NewUserFormState, value: string) {
    setNewUserForm((current) => ({ ...current, [field]: value }));
  }

  const statusText =
    email && !error
      ? isCheckingUser
        ? "Checking workspace profile..."
        : ""
      : "";

  return (
    <AppShell centered showFooter={false}>
      <div className="mx-auto flex w-full max-w-[440px] flex-col gap-lg">
        {step === "email" ? (
          <LoginCard
            email={email}
            error={error}
            isChecking={isCheckingUser}
            onEmailChange={setEmail}
            onSubmit={handleEmailSubmit}
            statusText={statusText}
          />
        ) : (
          <NewUserSetupCard
            email={email}
            error={error}
            form={newUserForm}
            isCreating={isCreatingUser}
            onBack={() => {
              setError("");
              setStep("email");
            }}
            onChange={updateNewUserField}
            onSubmit={handleCreateUser}
          />
        )}
      </div>
    </AppShell>
  );
}

type NewUserSetupCardProps = {
  email: string;
  error: string;
  form: NewUserFormState;
  isCreating: boolean;
  onBack: () => void;
  onChange: (field: keyof NewUserFormState, value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
};

function NewUserSetupCard({
  email,
  error,
  form,
  isCreating,
  onBack,
  onChange,
  onSubmit,
}: NewUserSetupCardProps) {
  return (
    <section className="glass-panel mx-auto w-full max-w-[440px] rounded-[8px] p-xl">
      <div className="space-y-xs">
        <p className="type-label text-on-surface-variant">New Workspace Profile</p>
        <h1 className="type-h1 text-text-main">Finish setup</h1>
        <p className="type-body-sm text-secondary">{email}</p>
      </div>

      <form className="mt-xl space-y-lg" onSubmit={onSubmit}>
        <div className="grid gap-md sm:grid-cols-2">
          <FormField
            autoComplete="given-name"
            icon={<Icon className="h-5 w-5" name="personSearch" />}
            id="first-name"
            label="First name"
            onChange={(value) => onChange("firstName", value)}
            placeholder="Rohan"
            value={form.firstName}
          />
          <FormField
            autoComplete="family-name"
            icon={<Icon className="h-5 w-5" name="personSearch" />}
            id="last-name"
            label="Last name"
            onChange={(value) => onChange("lastName", value)}
            placeholder="Gambhir"
            value={form.lastName}
          />
        </div>

        <FormField
          action={
            <a
              className="inline-flex items-center gap-xs text-[12px] font-semibold tracking-[0.05em] text-primary-container transition-colors hover:text-primary"
              href="https://platform.openai.com/api-keys"
              rel="noreferrer"
              target="_blank"
            >
              <Icon className="h-3.5 w-3.5" name="help" />
              API keys
            </a>
          }
          autoComplete="off"
          icon={<Icon className="h-5 w-5" name="key" />}
          id="api-key"
          label="OpenAI API key"
          onChange={(value) => onChange("apiKey", value)}
          placeholder="sk-..."
          type="password"
          value={form.apiKey}
        />

        <FormField
          autoComplete="organization-title"
          icon={<Icon className="h-5 w-5" name="shield" />}
          id="role"
          label="Role"
          onChange={(value) => onChange("role", value)}
          placeholder="Analyst"
          value={form.role}
        />

        {error ? <p className="px-xs text-[14px] font-medium text-error">{error}</p> : null}

        <div className="grid gap-sm sm:grid-cols-[auto_1fr]">
          <button
            className="rounded-full px-lg py-md text-[14px] font-semibold text-secondary transition hover:bg-surface-container-high hover:text-on-surface focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
            onClick={onBack}
            type="button"
          >
            Back
          </button>
          <QuarryButton disabled={isCreating} icon={<Icon className="h-[18px] w-[18px]" name="arrowRight" />} type="submit">
            {isCreating ? "Creating..." : "Create Profile"}
          </QuarryButton>
        </div>
      </form>
    </section>
  );
}
