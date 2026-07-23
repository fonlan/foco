import { GitBranch, Plus } from "lucide-react";
import { FormEvent } from "react";

import { useI18n } from "../../shared/i18n";
import {
  Button,
  FieldError,
  Input,
  Label,
  Modal,
  Spinner,
  TextField,
} from "../../shared/ui";

export function GitBranchDialog({
  branchName,
  error,
  isSaving,
  onBranchNameChange,
  onClose,
  onSubmit,
}: {
  branchName: string;
  error: string | null;
  isSaving: boolean;
  onBranchNameChange: (value: string) => void;
  onClose: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  const { t } = useI18n();
  return (
    <Modal.Backdrop isDismissable isOpen onOpenChange={(open) => !open && onClose()}>
      <Modal.Container placement="center" size="sm">
        <Modal.Dialog aria-label={t("New branch")}>
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Icon className="bg-accent-soft text-accent-soft-foreground">
              <GitBranch aria-hidden="true" className="size-5" />
            </Modal.Icon>
            <Modal.Heading>{t("New branch")}</Modal.Heading>
          </Modal.Header>
          <form
            onSubmit={(event) => {
              event.preventDefault();
              onSubmit(event);
            }}
          >
            <Modal.Body className="space-y-4">
              <TextField
                fullWidth
                isInvalid={Boolean(error)}
                name="git-branch-name"
                value={branchName}
                onChange={onBranchNameChange}
              >
                <Label>{t("Branch name")}</Label>
                <Input autoComplete="off" placeholder="feature/name" />
                {error ? <FieldError>{error}</FieldError> : null}
              </TextField>
            </Modal.Body>
            <Modal.Footer>
              <Button
                aria-label={t("Cancel branch creation")}
                isDisabled={isSaving}
                type="button"
                variant="tertiary"
                onPress={onClose}
              >
                {t("Cancel")}
              </Button>
              <Button
                aria-label={t("Create branch")}
                isDisabled={!branchName.trim()}
                isPending={isSaving}
                type="submit"
              >
                {({ isPending }) => (
                  <>
                    {isPending ? (
                      <Spinner color="current" size="sm" />
                    ) : (
                      <Plus aria-hidden="true" className="size-4" />
                    )}
                    {t("Create branch")}
                  </>
                )}
              </Button>
            </Modal.Footer>
          </form>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
