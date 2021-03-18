import { PartyBusEvent } from "@/api/partyBus/partyBusEvent";
import { INodeObject } from "@/api/sdf/dal/editorDal";
import { Node } from "@/api/sdf/model/node";

const NAME = "NodeUpdated";

interface IConstructor {
  node: Node;
  object: INodeObject;
}

export class NodeUpdatedEvent extends PartyBusEvent {
  node: Node | null;
  object: INodeObject | null;

  constructor(payload: IConstructor) {
    super(NAME);
    this.node = payload.node;
    this.object = payload.object;
  }

  static eventName(): string {
    return NAME;
  }
}
