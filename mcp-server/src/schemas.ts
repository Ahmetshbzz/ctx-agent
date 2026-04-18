import { z } from "zod";

export const ProjectPathSchema = z.object({
    project_path: z
        .string()
        .describe("Absolute path to the project root directory"),
});
