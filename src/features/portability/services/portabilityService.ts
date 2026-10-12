import { invokeCommand } from '@/lib/tauri/commands';
import type {
  ExportedFileDto,
  ExportTargetDto,
  ImportPreviewDto,
  ImportReportDto,
  ImportRequestDto,
  PickedFileDto,
} from '@/lib/tauri/commands';

/**
 * The UI's only door to the Rust core for sharing configurations. The files are chosen in the
 * operating system's own dialogs, opened by the core: the UI never names a path, it only ever
 * holds a file's text.
 */

/** The chosen configurations as the text of a file, and what to know before sharing it. */
export function exportConfigs(
  targets: ExportTargetDto[],
  includeDependencies: boolean,
): Promise<ExportedFileDto> {
  return invokeCommand('export_configs', { targets, includeDependencies });
}

/** Asks where to save the file. `null` when the person cancels; otherwise the file's name. */
export function saveConfigFile(fileName: string, text: string): Promise<string | null> {
  return invokeCommand('save_config_file', { fileName, text });
}

/** Asks which file to import. `null` when the person cancels. */
export function pickConfigFile(): Promise<PickedFileDto | null> {
  return invokeCommand('pick_config_file');
}

/** What importing would do here. Changes nothing; can take a moment (it asks the runtimes). */
export function previewConfigImport(
  text: string,
  workspaceId: string | null,
): Promise<ImportPreviewDto> {
  return invokeCommand('preview_config_import', { text, workspaceId });
}

/** Imports as decided: all of it, or nothing. */
export function importConfigs(text: string, request: ImportRequestDto): Promise<ImportReportDto> {
  return invokeCommand('import_configs', { text, request });
}
