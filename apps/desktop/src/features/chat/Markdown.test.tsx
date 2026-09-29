import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { Markdown } from "./Markdown";
it("renders formatting while blocking HTML execution and remote resources", () => {
  const { container } = render(
    <Markdown
      text={
        '**Safe**\n\n<script>alert(1)</script>\n\n[unsafe](javascript:alert(1))\n\n![remote](https://tracker.invalid/image)\n\n```js\nconsole.log("hello");\n```'
      }
    />,
  );
  expect(screen.getByText("Safe").tagName).toBe("STRONG");
  expect(container.querySelector("script,iframe,img,a[href]")).toBeNull();
  expect(screen.getByRole("button", { name: "Copy code" })).toBeInTheDocument();
});

it("opens a local citation viewer without creating an external link", () => {
  const { container } = render(
    <>
      <Markdown text="Supporting answer [1]." sourcePrefix="message" sourceCount={1} />
      <details id="source-message-1">
        <summary>Station notes</summary>
        <p>The station opens at 9 AM.</p>
      </details>
    </>,
  );
  fireEvent.click(screen.getByRole("button", { name: "Open source 1" }));
  expect(container.querySelector("details")).toHaveAttribute("open");
  expect(container.querySelector("a[href]")).toBeNull();
});
