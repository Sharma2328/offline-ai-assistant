import { useState, type ReactElement } from "react";
import { useNavigate } from "react-router-dom";
import {
  ArrowRight,
  Boxes,
  Check,
  FileText,
  MessageSquare,
  ShieldCheck,
  Sparkles,
} from "lucide-react";
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
 * model guidance → offline-lock confirmation. Step position is ephemeral UI
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
    <div className="onboarding-shell">
      <div>
        <div className="app-brand !mb-12 !px-0">
          <span className="brand-mark">
            <Sparkles aria-hidden="true" />
          </span>
          <div>
            <span className="brand-name">Offline</span>
            <span className="brand-caption">AI ASSISTANT</span>
          </div>
        </div>
        <p className="page-eyebrow">Intelligence, a little closer</p>
        <h1 className="welcome-title max-w-md !text-5xl">
          Big ideas.
          <br />
          Right here.
        </h1>
        <p className="mt-5 max-w-sm text-sm leading-7 text-muted-foreground">
          A quiet space to think, create, and explore with AI. Powered by your computer. Built
          around your privacy.
        </p>
        <div className="mt-8 space-y-4">
          {[
            { icon: MessageSquare, text: "Conversations that stay with you" },
            { icon: FileText, text: "Answers from your own documents" },
            { icon: ShieldCheck, text: "Local models. No cloud required." },
          ].map(({ icon: Icon, text }) => (
            <p key={text} className="flex items-center gap-3 text-xs text-muted-foreground">
              <Icon className="h-4 w-4 text-primary" aria-hidden="true" />
              {text}
            </p>
          ))}
        </div>
      </div>
      <section aria-labelledby="onboarding-heading">
        <div className="mb-5 flex items-center justify-between">
          <p className="text-xs font-medium text-muted-foreground">LET’S GET YOU SETTLED</p>
          <p className="text-xs text-muted-foreground">
            Step {String(stepIndex + 1)} of {String(STEPS.length)}
          </p>
        </div>
        <div className="mb-6 flex gap-2" aria-hidden="true">
          {STEPS.map((item, index) => (
            <span
              key={item}
              className={`onboarding-step ${index <= stepIndex ? "onboarding-step-active" : ""}`}
            />
          ))}
        </div>
        <Card>
          <CardHeader>
            <CardTitle id="onboarding-heading">{STEP_TITLES[step]}</CardTitle>
            <CardDescription>A few simple steps, then the space is yours.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <StepBody step={step} />

            {complete.isError ? (
              <p role="alert" className="text-sm text-destructive">
                Could not save your progress: {complete.error.message}
              </p>
            ) : null}

            <div className="flex justify-between border-t pt-5">
              <Button type="button" variant="outline" onClick={goBack} disabled={isFirst}>
                Back
              </Button>
              <Button type="button" onClick={goNext} disabled={complete.isPending}>
                {isLast ? (complete.isPending ? "Finishing…" : "Get started") : "Next"}
                {isLast ? <Check aria-hidden="true" /> : <ArrowRight aria-hidden="true" />}
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
          icon={Boxes}
          title="Add a model later"
          description="After setup, open Models to import an instruction-tuned GGUF file from your computer. We’ll check its memory requirements before you load it. You can finish setup without one."
        />
      );
    case "offline-lock":
      return (
        <div className="space-y-3 text-sm leading-relaxed">
          <p>
            The <strong>offline lock</strong> is <strong>on by default</strong>. Your models run on
            this computer, using only a local runtime connection. Your conversations and documents
            stay here.
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
