import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import {
  FormField,
  FieldErrors,
  fieldErrorsFrom,
  fieldsFrom,
} from "@/components/ui/form-field";
import { ApiError } from "@/lib/api-client";

/** W-14 表单协议：FormField 的 aria 关联（label/错误/hint/必填）与
 *  FieldErrors 的后端信封渲染（含点字段名聚焦） */

describe("FormField", () => {
  it("label 经 htmlFor 关联控件，错误经 aria-describedby 关联错误节点", () => {
    render(
      <FormField
        name="username"
        label="用户名"
        error="已被占用"
        required
      >
        <input />
      </FormField>,
    );
    const input = screen.getByLabelText("用户名*");
    // id 规则：控件 field-<name>，错误节点 field-<name>-err
    expect(input).toHaveAttribute("id", "field-username");
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input.getAttribute("aria-describedby")).toBe(
      "field-username-err",
    );
    expect(screen.getByText("已被占用")).toHaveAttribute(
      "id",
      "field-username-err",
    );
  });

  it("无错误时 hint 占据 describedby（hint 让位语义）；无二者不挂", () => {
    const { rerender } = render(
      <FormField name="email" label="邮箱" hint="选填">
        <input />
      </FormField>,
    );
    // 非必填时 label 内含「选填」标记，按子串匹配
    const input = screen.getByLabelText(/邮箱/);
    expect(input).toHaveAttribute(
      "aria-describedby",
      "field-email-err",
    );
    rerender(
      <FormField name="email" label="邮箱">
        <input />
      </FormField>,
    );
    expect(screen.getByLabelText(/邮箱/)).not.toHaveAttribute(
      "aria-describedby",
    );
  });

  it("required 织入 aria-required；非必填展示「选填」标记", () => {
    render(
      <FormField name="password" label="密码" required>
        <input type="password" />
      </FormField>,
    );
    expect(screen.getByLabelText("密码*")).toHaveAttribute(
      "aria-required",
      "true",
    );
  });
});

describe("FieldErrors", () => {
  it("渲染每字段一组错误行（多值合并），空 map 不渲染", () => {
    const { container, rerender } = render(
      <FieldErrors
        fields={{
          username: ["已被占用", "含非法字符"],
          email: "格式无效",
        }}
      />,
    );
    const box = container.querySelector(".ui-ferr");
    expect(box).toHaveAttribute("role", "alert");
    expect(screen.getByText("已被占用；含非法字符")).toBeInTheDocument();
    expect(screen.getByText("格式无效")).toBeInTheDocument();
    rerender(<FieldErrors fields={{ username: [] }} />);
    expect(container.querySelector(".ui-ferr")).toBeNull();
  });

  it("点字段名聚焦对应控件（id 规则 field-<name>）", async () => {
    render(
      <>
        <FormField name="username" label="用户名">
          <input />
        </FormField>
        <FieldErrors fields={{ username: "已被占用" }} />
      </>,
    );
    await userEvent.click(screen.getByRole("button", { name: "username" }));
    expect(screen.getByLabelText(/用户名/)).toHaveFocus();
  });
});

describe("fieldErrorsFrom / fieldsFrom（后端 1002 信封）", () => {
  it("ApiError(1002) 数组信封 → 字段 map；同字段多错误聚合", () => {
    const e = new ApiError(
      1002,
      "参数校验失败: 2 个字段未通过校验",
      [
        { field: "descr", error: "描述太短" },
        { field: "descr", error: "含违禁词" },
        { field: "category_id", error: "分类缺失" },
      ],
      400,
    );
    expect(e.fields).toEqual({
      descr: ["描述太短", "含违禁词"],
      category_id: ["分类缺失"],
    });
    expect(fieldErrorsFrom(e)).toEqual(e.fields);
  });

  it("非字段错误（其他码 / 无 data）返回 null", () => {
    expect(fieldErrorsFrom(new ApiError(4001, "余额不足"))).toBeNull();
    expect(
      fieldErrorsFrom(new ApiError(1002, "参数校验失败", null)),
    ).toBeNull();
    expect(fieldErrorsFrom(new Error("网络异常"))).toBeNull();
  });

  it("fieldsFrom 兼容对象形态 {field: [msgs]}", () => {
    expect(fieldsFrom({ descr: ["描述太短"] })).toEqual({
      descr: ["描述太短"],
    });
    expect(fieldsFrom([{ field: "name", error: "必填" }])).toEqual({
      name: "必填",
    });
  });
});
