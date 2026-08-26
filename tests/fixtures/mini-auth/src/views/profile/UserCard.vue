<template>
  <div class="user-card">{{ name }}</div>
</template>
<script>
export default {
  name: "UserCard",
  props: {
    name: { type: String, default: "" },
  },
};
</script>
