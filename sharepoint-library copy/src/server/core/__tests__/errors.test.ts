import { describe, expect, test } from "bun:test";
import { SharePointClientError } from "../errors";

describe("SharePointClientError", () => {
  test("creates error with default status code 513", () => {
    const error = new SharePointClientError("test");
    expect(error.message).toBe("test");
    expect(error.statusCode).toBe(513);
    expect(error.details).toBeUndefined();
    expect(error.name).toBe("SharePointClientError");
  });

  test("accepts custom status code and details", () => {
    const error = new SharePointClientError("not found", 404, { id: "abc" });
    expect(error.statusCode).toBe(404);
    expect(error.details).toEqual({ id: "abc" });
  });

  test("is an instance of Error", () => {
    const error = new SharePointClientError("test");
    expect(error).toBeInstanceOf(Error);
    expect(error).toBeInstanceOf(SharePointClientError);
  });

  test("has a stack trace", () => {
    const error = new SharePointClientError("test");
    expect(error.stack).toBeDefined();
  });

  test("can be caught as SharePointClientError", () => {
    try {
      throw new SharePointClientError("thrown", 500);
    } catch (e) {
      expect(e).toBeInstanceOf(SharePointClientError);
    }
  });
});
