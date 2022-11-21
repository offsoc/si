// This is a map of valid websocket events to the shape of their payload
// used in the subscribe fn to limit valid event names and set callback payload type

import { FixStatus } from "../fixes/fixes.store";
import { GlobalUpdateStatus, ComponentUpdateStatus } from "../status.store";

// TODO: a few of these use the same id objects (ex: componentId)
// but in a few cases the changeset ID may have been accidentally left out?
// once things are working again, we should do a big review of all the realtime events coming from the backend...

export type WsEventPayloadMap = {
  ChangeSetCreated: number;
  ChangeSetApplied: number;
  ChangeSetWritten: number;
  ChangeSetCancelled: number;

  CheckedQualifications: {
    prototypeId: number;
    componentId: number;
  };

  CodeGenerated: {
    componentId: number;
  };

  ConfirmationStatusUpdate: {
    componentId: number;
    confirmationPrototypeId: number;
    status: "success" | "running" | "pending" | "failure" | "error";
    errorMessage?: string;
  };

  // NOT CURRENTLY USED - but leaving here so we remember these events exist
  // SecretCreated: number;
  // ResourceRefreshed: {
  //   componentId: number;
  // }
  // UpdatedDependentValue: {
  //   componentId: number;
  // }
  // CommandOutput: { runId: number; output: string }
  // CommandReturn: {
  //   runId: number;
  //   createdResources: Resource[];
  //   updatedResources: Resource[];
  //   output: string[];
  //   runnerState: WorkflowRunnerState;
  // };
  FixReturn: {
    id: number;
    batchId: number;
    confirmationResolverId: number;
    action: string;
    output: string[];
    status: FixStatus;
  };
  FixBatchReturn: {
    id: number;
    status: FixStatus;
  };

  UpdateStatus: {
    global: GlobalUpdateStatus;
    components?: ComponentUpdateStatus[];
  };
};
