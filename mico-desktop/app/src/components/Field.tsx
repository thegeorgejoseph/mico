import { forwardRef } from "react";
import type { InputHTMLAttributes, SelectHTMLAttributes, TextareaHTMLAttributes } from "react";

export const TextField = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(function TextField(props, ref) {
  return <input className="ui-field" ref={ref} {...props} />;
});

export const SelectField = forwardRef<HTMLSelectElement, SelectHTMLAttributes<HTMLSelectElement>>(function SelectField(props, ref) {
  return <select className="ui-field" ref={ref} {...props} />;
});

export const TextAreaField = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement>>(function TextAreaField(props, ref) {
  return <textarea className="ui-field ui-field--textarea" ref={ref} {...props} />;
});
