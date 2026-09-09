import { useState, type ReactElement } from "react";
import { useNavigate } from "react-router-dom";
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  EmptyState,
  ErrorState,
} from "@offline-ai/ui";

import { HardwareSummary } from "@features/system/HardwareSummary";
import { useSystemInspection } from "@features/system/useSystemInspection";
import { useCompleteOnboarding } from "./useCompleteOnboarding";

const STEPS = ["welcome", "hardware", "model", "offline-lock"] as const;
type Step = (typeof STEPS)[number];

const STEP_TITLES: Record<Step, string> = {
  welcome: "Welcome",
  hardware: "Your hardware",
  model: "Add a model",
  "offline-lock": "You're offline by default",
};

/**
 * First-run onboarding wizard (FR-ONB-001…003). Fully local: welcome → detected hardware →
 * model (deferred to Phase 4) → offline-lock confirmation. Step position is ephemeral UI
 * state; data fetching and the completion write live in feature hooks (doc §5/§8).
 */
export function OnboardingPage(): ReactElement {
  const navigate = useNavigate();
  const [stepIndex, setStepIndex] = useState(0);
  const step: Step = STEPS[stepIndex] ?? "welcome";

  const complete = useCompleteOnboarding();

  const isFirst = stepIndex === 0;
  const isLast = stepIndex === STEPS.length - 1;

  const goNext = (): void => {
    if (isLast) {
      complete.mutate(undefined, {
        onSuccess: () => {
          void navigate("/", { replace: true });
        },
      });
      return;
    }
    setStepIndex((index) => Math.min(index + 1, STEPS.length - 1));
  };

  const goBack = (): void => {
    setStepIndex((index) => Math.max(index - 1, 0));
  };

  return (
    <div className="mx-auto flex min-h-screen w-full max-w-2xl flex-col justify-center p-8">
      <section aria-labelledby="onboarding-heading">
        <p className="mb-2 text-sm text-muted-foreground">
          Step {String(stepIndex + 1)} of {String(STEPS.length)}
        </p>
        <Card>
          <CardHeader>
            <CardTitle id="onboarding-heading">{STEP_TITLES[step]}</CardTitle>
            <CardDescription>
              Set up the Offline AI Assistant. Nothing leaves this device.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <StepBody step={step} />

            {complete.isError ? (
              <p role="alert" className="text-sm text-destructive">
                Could not save your progress: {complete.error.message}
              </p>
            ) : null}

            <div className="flex justify-between">
              <Button type="button" variant="outline" onClick={goBack} disabled={isFirst}>
                Back
              </Button>
              <Button type="button" onClick={goNext} disabled={complete.isPending}>
                {isLast ? (complete.isPending ? "Finishing…" : "Get started") : "Next"}
              </Button>
            </div>
          </CardContent>
        </Card>
      </section>
    </div>
  );
}

function StepBody({ step }: { step: Step }): ReactElement {
  switch (step) {
    case "welcome":
      return (
        <p className="text-sm leading-relaxed">
          This assistant runs AI models entirely on your machine — chat, document search, and model
          benchmarking, with no cloud services and no data ever sent off the device. Let&apos;s make
          sure it&apos;s a good fit for your hardware.
        </p>
      );
    case "hardware":
      return <HardwareStep />;
    case "model":
      return (
        <EmptyState
          title="Add a model later"
          description="Importing and loading models arrives in the next release stage. You can finish setup now and add a model from the Models screen when it's ready."
        />
      );
    case "offline-lock":
      return (
        <div className="space-y-3 text-sm leading-relaxed">
          <p>
            The <strong>offline lock</strong> is <strong>on by default</strong>. While it&apos;s on,
            the app makes no network connections at all — everything happens locally.
          </p>
          <p className="text-muted-foreground">
            You can review this later in Settings, but leaving it on is strongly recommended for a
            fully private, offline experience.
          </p>
        </div>
      );
    default:
      return <p className="text-sm">Unknown step.</p>;
  }
}

function HardwareStep(): ReactElement {
  const { data, isLoading, isError, error, refetch } = useSystemInspection();

  if (isLoading) {
    return (
      <p role="status" className="text-sm text-muted-foreground">
        Inspecting your hardware…
      </p>
    );
  }

  if (isError) {
    return (
      <ErrorState
        title="Couldn't inspect hardware"
        message={error.message}
        recovery={error.recovery}
        code={error.code}
        action={
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => {
              void refetch();
            }}
          >
            Try again
          </Button>
        }
      />
    );
  }

  if (!data) {
    return (
      <p role="status" className="text-sm text-muted-foreground">
        No hardware information available.
      </p>
    );
  }

  return <HardwareSummary inspection={data} />;
}
