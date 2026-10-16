import type { StorageStatus } from "../bindings/StorageStatus";
import { invokeCommand } from "./invoke";

/** État du stockage local chiffré, déterminé au démarrage. */
export function getStorageStatus(): Promise<StorageStatus> {
  return invokeCommand<StorageStatus>("storage_status");
}
