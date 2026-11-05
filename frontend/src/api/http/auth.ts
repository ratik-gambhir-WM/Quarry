import type {
  LoginInput,
  RegisterInput,
  SessionResponse,
  CurrentUserResponse,
  QuarryApi,
} from "../../contracts/quarryApi";

import { requestJson } from "./transport";

export const httpAuthApi: Pick<
  QuarryApi,
  | "login"
  | "register"
  | "currentUser"
  | "logout"
> = {
  login: (input: LoginInput) =>
    requestJson<SessionResponse>(
      "/api/v1/auth/login",
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(input),
      },
      undefined,
      false,
    ),
  register: (input: RegisterInput) =>
    requestJson<SessionResponse>(
      "/api/v1/auth/register",
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(input),
      },
      undefined,
      false,
    ),
  currentUser: () =>
    requestJson<CurrentUserResponse>(
      "/api/v1/auth/me",
      undefined,
      undefined,
      false,
    ),
  logout: () =>
    requestJson<void>(
      "/api/v1/auth/logout",
      { method: "POST" },
      undefined,
      false,
    ),
};
