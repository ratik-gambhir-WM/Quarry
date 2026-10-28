import type { SessionResponse } from "../../contracts/quarryApi";
export const testSession: SessionResponse = {
  accessToken: "a".repeat(43), tokenType: "Bearer", expiresAt: new Date(Date.now() + 3_600_000).toISOString(),
  user: { id: 1, username: "analyst", email: "analyst@example.com", firstName: "Alex", lastName: "Morgan", jobTitle: "Analyst", createdAt: "2026-10-28T00:00:00Z", updatedAt: "2026-10-28T00:00:00Z" },
};
