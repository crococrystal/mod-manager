export function Button({ tone = 'default', icon: Icon, children, className = '', ...props }) {
  return (
    <button className={`button ${tone ? `button-${tone}` : ''} ${className}`.trim()} type="button" {...props}>
      {Icon ? <Icon size={17} /> : null}
      {children ? <span>{children}</span> : null}
    </button>
  );
}

export function IconButton({ icon: Icon, label, className = '', ...props }) {
  return (
    <button className={`iconButton ${className}`.trim()} type="button" title={label} aria-label={label} {...props}>
      <Icon size={18} />
    </button>
  );
}
