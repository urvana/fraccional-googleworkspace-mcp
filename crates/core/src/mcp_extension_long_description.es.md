Conecta Claude con tu Google Workspace de Fraccional: Gmail, Calendar, Drive, Chat, Sheets, Forms, Docs, Slides, Meet, Tasks y Contactos.

**Qué le puedes pedir**
- "Resume mis correos sin leer y responde el de finanzas." (requiere Escritura en Gmail)
- "¿Qué tengo hoy? Prepárame la próxima reunión con la agenda y los documentos."
- "Agrega esta fila a la planilla de gastos."
- "Convierte este correo en una tarea."
- "Envía un mensaje al espacio del equipo en Google Chat."
- "Hazme el resumen de la semana: reuniones y correos sin leer."

**Lo que el MCP de Google no hace (a noviembre de 2026)**
- Enviar, responder y reenviar correos. El de Google solo crea borradores.
- Meet, Forms, Tasks y reportes de Admin, que no tienen MCP de Google.
- Flujos entre servicios: standup, preparación de reuniones, correo a tarea y resumen semanal.
- Sin inscribirte en el Developer Preview ni crear un proyecto de Google Cloud: instalas la app y conectas tu cuenta.
- Corre en tu equipo, no en un servidor remoto.

**Cómo funciona**
- Solo funciona con cuentas @fraccional.com.
- Necesita la app fraccional-googleworkspace-mcp instalada en tu equipo, con tu cuenta conectada.
- Tú controlas los permisos en la app: Lectura o Escritura por servicio, y puedes revocarlos cuando quieras.
- fraccional-googleworkspace-mcp no tiene servidores. Tu sesión de Google queda cifrada en tu equipo, con la clave en el llavero del sistema. Lo que Claude lee con estas herramientas pasa a tu conversación con Claude.
- Por debajo usa gws, la CLI open source de Google Workspace ([github.com/googleworkspace/cli](https://github.com/googleworkspace/cli)), fijada en la v0.22.5.